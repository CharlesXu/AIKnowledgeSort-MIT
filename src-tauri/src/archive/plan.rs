use crate::discovery::ReviewedSource;
use crate::identity::ContentIdentity;
use crate::naming::schema::{canonical_policy, NamingDecisionEvidence, NamingStatus};
use crate::naming::NamingBatch;
use crate::vault::VaultSummary;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PLAN_VERSION: u32 = 2;
const MAX_OPAQUE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlanItem {
    pub item_id: String,
    pub source_path: String,
    pub destination_path: String,
    pub original_name: String,
    pub canonical_name: String,
    pub naming: NamingDecisionEvidence,
    pub byte_size: u64,
    pub identity: ContentIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlan {
    pub plan_id: String,
    pub plan_version: u32,
    pub proposal_id: String,
    pub naming_batch_id: String,
    pub authority_id: String,
    pub vault_path: String,
    pub expires_at_unix_ms: u64,
    pub confirmation_nonce: String,
    pub source_preserved: bool,
    pub items: Vec<ArchivePlanItem>,
}

#[derive(Clone, Copy)]
pub(crate) struct PlanRegistryLimits {
    pub max_plans: usize,
    pub max_items: usize,
    pub ttl: Duration,
}

impl Default for PlanRegistryLimits {
    fn default() -> Self {
        Self {
            max_plans: 32,
            max_items: 1_000,
            ttl: Duration::from_secs(5 * 60),
        }
    }
}

struct StoredPlan {
    expires_at: Instant,
    plan: ArchivePlan,
}

#[derive(Clone)]
pub struct ArchivePlanRegistry {
    limits: PlanRegistryLimits,
    plans: Arc<Mutex<HashMap<String, StoredPlan>>>,
}

impl Default for ArchivePlanRegistry {
    fn default() -> Self {
        Self::new(PlanRegistryLimits::default())
    }
}

impl ArchivePlanRegistry {
    pub(crate) fn new(limits: PlanRegistryLimits) -> Self {
        Self {
            limits,
            plans: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn create_named_at(
        &self,
        proposal_id: &str,
        sources: Vec<ReviewedSource>,
        naming_batch: NamingBatch,
        vault: VaultSummary,
        now: Instant,
        wall_time: SystemTime,
    ) -> Result<ArchivePlan, String> {
        validate_opaque_id(proposal_id, "discovery proposal")?;
        validate_opaque_id(&naming_batch.batch_id, "naming batch")?;
        let policy = canonical_policy();
        if naming_batch.discovery_proposal_id != proposal_id {
            return Err("Naming batch does not match the discovery proposal".to_owned());
        }
        if naming_batch.policy_id != policy.policy_id
            || naming_batch.policy_version != policy.version
        {
            return Err("Naming batch policy is not supported".to_owned());
        }
        if sources.is_empty()
            || sources.len() > self.limits.max_items
            || sources.len() != naming_batch.proposals.len()
        {
            return Err("Named archive selection is empty, mismatched, or too large".to_owned());
        }
        validate_opaque_id(&vault.authority_id, "Vault authority")?;

        let mut proposals = naming_batch
            .proposals
            .into_iter()
            .map(|proposal| (proposal.item_id.clone(), proposal))
            .collect::<HashMap<_, _>>();
        if proposals.len() != sources.len() {
            return Err("Naming batch contains duplicate items".to_owned());
        }

        let mut seen_item_ids = HashSet::with_capacity(sources.len());
        let mut seen_destinations = HashSet::with_capacity(sources.len());
        let mut items = Vec::with_capacity(sources.len());
        for source in sources {
            validate_opaque_id(&source.item_id, "reviewed item")?;
            if !seen_item_ids.insert(source.item_id.clone()) {
                return Err("Archive plan contains duplicate reviewed items".to_owned());
            }
            source.identity.validate()?;
            let proposal = proposals
                .remove(&source.item_id)
                .ok_or_else(|| "Naming batch is missing a reviewed item".to_owned())?;
            if proposal.status != NamingStatus::Proposed
                || proposal.review_reason.is_some()
                || proposal.original_name != source.name
                || proposal.identity != source.identity
                || proposal.policy_id != naming_batch.policy_id
                || proposal.policy_version != naming_batch.policy_version
            {
                return Err(
                    "Naming proposal is not approved or does not match the reviewed source"
                        .to_owned(),
                );
            }
            let canonical_name = proposal
                .canonical_name
                .clone()
                .ok_or_else(|| "Naming proposal requires review".to_owned())?;
            validate_original_name(&canonical_name)?;
            let destination = Path::new("Originals")
                .join(&source.identity.digest)
                .join(&canonical_name);
            if !seen_destinations.insert(destination.clone()) {
                return Err("Archive plan contains duplicate destinations".to_owned());
            }
            let naming = NamingDecisionEvidence {
                naming_proposal_id: proposal.proposal_id,
                original_name: source.name.clone(),
                canonical_name: canonical_name.clone(),
                policy_id: proposal.policy_id,
                policy_version: proposal.policy_version,
                applied_rule: proposal.applied_rule,
                facts: proposal.facts,
            };
            items.push(ArchivePlanItem {
                item_id: source.item_id,
                source_path: source.path.to_string_lossy().into_owned(),
                destination_path: destination.to_string_lossy().into_owned(),
                original_name: source.name,
                canonical_name,
                naming,
                byte_size: source.byte_size,
                identity: source.identity,
            });
        }
        items.sort_by(|left, right| left.item_id.cmp(&right.item_id));

        let expires_at_unix_ms = wall_time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is before the Unix epoch".to_owned())?
            .checked_add(self.limits.ttl)
            .ok_or_else(|| "Archive plan expiry overflowed".to_owned())?
            .as_millis()
            .try_into()
            .map_err(|_| "Archive plan expiry is out of range".to_owned())?;
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Archive plan registry is unavailable".to_owned())?;
        plans.retain(|_, stored| stored.expires_at > now);
        if plans.len() >= self.limits.max_plans {
            return Err("Too many active archive plans".to_owned());
        }
        let plan_id = unique_plan_id(&plans);
        let plan = ArchivePlan {
            plan_id: plan_id.clone(),
            plan_version: PLAN_VERSION,
            proposal_id: proposal_id.to_owned(),
            naming_batch_id: naming_batch.batch_id,
            authority_id: vault.authority_id,
            vault_path: vault.display_path,
            expires_at_unix_ms,
            confirmation_nonce: Uuid::new_v4().simple().to_string(),
            source_preserved: true,
            items,
        };
        plans.insert(
            plan_id,
            StoredPlan {
                expires_at: now + self.limits.ttl,
                plan: plan.clone(),
            },
        );
        Ok(plan)
    }

    pub(crate) fn consume_at(
        &self,
        plan_id: &str,
        confirmation_nonce: &str,
        now: Instant,
    ) -> Result<ArchivePlan, String> {
        validate_opaque_id(plan_id, "archive plan")?;
        validate_opaque_id(confirmation_nonce, "confirmation nonce")?;
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Archive plan registry is unavailable".to_owned())?;
        plans.retain(|_, stored| stored.expires_at > now);
        let stored = plans
            .get(plan_id)
            .ok_or_else(|| "Unknown, expired, or already consumed archive plan".to_owned())?;
        if stored.plan.confirmation_nonce != confirmation_nonce {
            return Err("Archive confirmation does not match the reviewed plan".to_owned());
        }
        plans
            .remove(plan_id)
            .map(|stored| stored.plan)
            .ok_or_else(|| "Archive plan could not be consumed".to_owned())
    }
}

fn validate_opaque_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_OPAQUE_ID_BYTES {
        Err(format!("Invalid {label} id"))
    } else {
        Ok(())
    }
}

fn validate_original_name(name: &str) -> Result<(), String> {
    let mut components = Path::new(name).components();
    if name.is_empty()
        || name.len() > 255
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || name.ends_with([' ', '.'])
        || name
            .chars()
            .any(|character| character.is_control() || r#"<>:"/\|?*"#.contains(character))
    {
        return Err("Original filename is not safe on all supported platforms".to_owned());
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value))
        || stem
            .strip_prefix("LPT")
            .and_then(|value| value.parse::<u8>().ok())
            .is_some_and(|value| (1..=9).contains(&value));
    if reserved {
        return Err("Original filename is reserved on Windows".to_owned());
    }
    Ok(())
}

fn unique_plan_id(plans: &HashMap<String, StoredPlan>) -> String {
    loop {
        let candidate = Uuid::new_v4().simple().to_string();
        if !plans.contains_key(&candidate) {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArchivePlanRegistry, PlanRegistryLimits};
    use crate::discovery::ReviewedSource;
    use crate::identity::ContentIdentity;
    use crate::naming::schema::{
        NamingFact, NamingFactKind, NamingProposal, NamingReviewReason, NamingStatus,
    };
    use crate::naming::NamingBatch;
    use crate::vault::{VaultStatus, VaultSummary};
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime};

    fn source(item_id: &str, name: &str) -> ReviewedSource {
        ReviewedSource {
            item_id: item_id.to_owned(),
            path: PathBuf::from(format!("/review/{name}")),
            name: name.to_owned(),
            byte_size: 12,
            identity: ContentIdentity {
                algorithm: "SHA-256".to_owned(),
                digest: "0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0"
                    .to_owned(),
            },
        }
    }

    fn vault() -> VaultSummary {
        VaultSummary {
            authority_id: "vault-authority".to_owned(),
            display_path: "/vault".to_owned(),
            status: VaultStatus::Authoritative,
        }
    }

    fn naming_batch(item_id: &str, original_name: &str, canonical_name: &str) -> NamingBatch {
        naming_batch_many(&[(item_id, original_name, canonical_name)])
    }

    fn naming_batch_many(entries: &[(&str, &str, &str)]) -> NamingBatch {
        NamingBatch {
            batch_id: "naming-batch".to_owned(),
            discovery_proposal_id: "reviewed-proposal".to_owned(),
            policy_id: "canonical-v1".to_owned(),
            policy_version: "1.0.0".to_owned(),
            expires_at_unix_ms: u64::MAX,
            proposals: entries
                .iter()
                .map(|(item_id, original_name, canonical_name)| NamingProposal {
                    proposal_id: format!("naming-proposal-{item_id}"),
                    item_id: (*item_id).to_owned(),
                    original_name: (*original_name).to_owned(),
                    canonical_name: Some((*canonical_name).to_owned()),
                    identity: source(item_id, original_name).identity,
                    policy_id: "canonical-v1".to_owned(),
                    policy_version: "1.0.0".to_owned(),
                    applied_rule: "ordered-cited-facts-v1".to_owned(),
                    status: NamingStatus::Proposed,
                    review_reason: None,
                    facts: vec![NamingFact {
                        kind: NamingFactKind::Subject,
                        value: "Reset reliability".to_owned(),
                        evidence_location: "page:1".to_owned(),
                    }],
                })
                .collect(),
        }
    }

    #[test]
    fn binds_the_canonical_name_from_one_exact_naming_batch() {
        let now = Instant::now();
        let registry = ArchivePlanRegistry::default();
        let plan = registry
            .create_named_at(
                "reviewed-proposal",
                vec![source("item-one", "000123.md")],
                naming_batch("item-one", "000123.md", "Reset-reliability.md"),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .expect("create named archive plan");

        assert_eq!(plan.naming_batch_id, "naming-batch");
        assert_eq!(plan.items[0].original_name, "000123.md");
        assert_eq!(plan.items[0].canonical_name, "Reset-reliability.md");
        assert_eq!(
            plan.items[0].destination_path,
            "Originals/0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0/Reset-reliability.md"
        );
        assert_eq!(plan.items[0].naming.policy_id, "canonical-v1");
    }

    #[test]
    fn creates_an_exact_source_preserving_plan() {
        let now = Instant::now();
        let registry = ArchivePlanRegistry::default();
        let plan = registry
            .create_named_at(
                "reviewed-proposal",
                vec![source("item-one", "report.md")],
                naming_batch("item-one", "report.md", "report.md"),
                vault(),
                now,
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
            )
            .expect("create archive plan");

        assert_eq!(plan.plan_version, 2);
        assert_eq!(plan.proposal_id, "reviewed-proposal");
        assert_eq!(plan.authority_id, "vault-authority");
        assert!(plan.source_preserved);
        assert!(!plan.plan_id.is_empty());
        assert!(!plan.confirmation_nonce.is_empty());
        assert_eq!(plan.items[0].source_path, "/review/report.md");
        assert_eq!(
            plan.items[0].destination_path,
            "Originals/0d764ea993d0f614fb0dc75e85a4cbbb815b7dd973a1778644c97d7a11a435c0/report.md"
        );
        assert_eq!(plan.items[0].identity.algorithm, "SHA-256");
    }

    #[test]
    fn consumes_only_one_exact_unexpired_confirmation() {
        let now = Instant::now();
        let registry = ArchivePlanRegistry::new(PlanRegistryLimits {
            max_plans: 2,
            max_items: 4,
            ttl: Duration::from_secs(5),
        });
        let plan = registry
            .create_named_at(
                "reviewed-proposal",
                vec![source("item-one", "report.md")],
                naming_batch("item-one", "report.md", "report.md"),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .expect("create archive plan");

        assert!(registry
            .consume_at(&plan.plan_id, "wrong-nonce", now)
            .is_err());
        assert!(registry
            .consume_at(
                &plan.plan_id,
                &plan.confirmation_nonce,
                now + Duration::from_secs(6),
            )
            .is_err());

        let fresh = registry
            .create_named_at(
                "reviewed-proposal",
                vec![source("item-two", "fresh.md")],
                naming_batch("item-two", "fresh.md", "fresh.md"),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .expect("create fresh plan");
        let consumed = registry
            .consume_at(&fresh.plan_id, &fresh.confirmation_nonce, now)
            .expect("consume exact confirmation");
        assert_eq!(consumed, fresh);
        assert!(registry
            .consume_at(&fresh.plan_id, &fresh.confirmation_nonce, now)
            .is_err());
    }

    #[test]
    fn rejects_duplicate_unsafe_and_invalid_identity_items() {
        let registry = ArchivePlanRegistry::default();
        let now = Instant::now();
        let duplicate = source("same", "one.md");
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![duplicate.clone(), duplicate],
                naming_batch_many(&[("same", "one.md", "one.md"), ("same", "one.md", "one.md")]),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![
                    source("same-destination-one", "same.md"),
                    source("same-destination-two", "same.md"),
                ],
                naming_batch_many(&[
                    ("same-destination-one", "same.md", "same.md"),
                    ("same-destination-two", "same.md", "same.md"),
                ]),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![source("unsafe", "CON.txt")],
                naming_batch("unsafe", "CON.txt", "CON.txt"),
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        let mut invalid = source("invalid", "valid.md");
        invalid.identity.algorithm = "sha256".to_owned();
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![invalid.clone()],
                NamingBatch {
                    proposals: vec![NamingProposal {
                        identity: invalid.identity.clone(),
                        ..naming_batch("invalid", "valid.md", "valid.md").proposals[0].clone()
                    }],
                    ..naming_batch("invalid", "valid.md", "valid.md")
                },
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
    }

    #[test]
    fn rejects_review_and_mismatched_naming_proposals() {
        let registry = ArchivePlanRegistry::default();
        let now = Instant::now();
        let reviewed_source = source("item-one", "000123.pdf");

        let mut review = naming_batch("item-one", "000123.pdf", "Report.pdf");
        review.proposals[0].status = NamingStatus::NamingReview;
        review.proposals[0].review_reason = Some(NamingReviewReason::MissingEvidence);
        review.proposals[0].canonical_name = None;
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![reviewed_source.clone()],
                review,
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());

        let mut mismatched = naming_batch("item-one", "different.pdf", "Report.pdf");
        mismatched.proposals[0].identity = reviewed_source.identity.clone();
        assert!(registry
            .create_named_at(
                "reviewed-proposal",
                vec![reviewed_source],
                mismatched,
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
    }
}

use super::proposal::{classify, ClassificationProposal, EvidencePacket, EvidenceReference};
use super::schema::{DeclarativeProfile, ProfileStatus};
use crate::discovery::ReviewedSource;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_OPAQUE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClassificationItemInput {
    pub item_id: String,
    pub references: Vec<EvidenceReference>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationBatchItem {
    pub item_id: String,
    pub proposal: ClassificationProposal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationBatch {
    pub batch_id: String,
    pub discovery_proposal_id: String,
    pub profile_id: String,
    pub profile_version: String,
    pub expires_at_unix_ms: u64,
    pub items: Vec<ClassificationBatchItem>,
}

struct StoredBatch {
    expires_at: Instant,
    batch: ClassificationBatch,
}

#[derive(Clone)]
pub struct ClassificationBatchRegistry {
    batches: Arc<Mutex<HashMap<String, StoredBatch>>>,
    ttl: Duration,
}

impl Default for ClassificationBatchRegistry {
    fn default() -> Self {
        Self {
            batches: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(5 * 60),
        }
    }
}

impl ClassificationBatchRegistry {
    pub(crate) fn create_at(
        &self,
        discovery_proposal_id: &str,
        profile: &DeclarativeProfile,
        sources: Vec<ReviewedSource>,
        inputs: Vec<ClassificationItemInput>,
        now: Instant,
        wall_time: SystemTime,
    ) -> Result<ClassificationBatch, String> {
        validate_id(discovery_proposal_id, "discovery proposal")?;
        profile.validate()?;
        if profile.status != ProfileStatus::Approved {
            return Err("Classification requires the active approved profile".to_owned());
        }
        if inputs.is_empty() || inputs.len() > 1_000 || inputs.len() != sources.len() {
            return Err("Classification selection is empty, mismatched, or too large".to_owned());
        }
        let mut source_by_id = HashMap::with_capacity(sources.len());
        for source in sources {
            validate_id(&source.item_id, "reviewed item")?;
            source.identity.validate()?;
            if source_by_id
                .insert(source.item_id.clone(), source)
                .is_some()
            {
                return Err("Classification contains duplicate reviewed sources".to_owned());
            }
        }
        let input_ids = inputs
            .iter()
            .map(|input| input.item_id.as_str())
            .collect::<HashSet<_>>();
        if input_ids.len() != inputs.len()
            || input_ids.len() != source_by_id.len()
            || input_ids
                .iter()
                .any(|item_id| !source_by_id.contains_key(*item_id))
        {
            return Err("Classification inputs do not match the reviewed selection".to_owned());
        }
        let mut items = Vec::with_capacity(inputs.len());
        for input in inputs {
            let source = source_by_id
                .remove(&input.item_id)
                .ok_or_else(|| "Classification reviewed source is missing".to_owned())?;
            let proposal = classify(
                profile,
                EvidencePacket {
                    source_identity: source.identity,
                    references: input.references,
                },
            )?;
            items.push(ClassificationBatchItem {
                item_id: input.item_id,
                proposal,
            });
        }
        items.sort_by(|left, right| left.item_id.cmp(&right.item_id));
        let expires_at_unix_ms = wall_time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is before the Unix epoch".to_owned())?
            .checked_add(self.ttl)
            .ok_or_else(|| "Classification expiry overflowed".to_owned())?
            .as_millis()
            .try_into()
            .map_err(|_| "Classification expiry is out of range".to_owned())?;
        let batch = ClassificationBatch {
            batch_id: Uuid::new_v4().simple().to_string(),
            discovery_proposal_id: discovery_proposal_id.to_owned(),
            profile_id: profile.profile_id.clone(),
            profile_version: profile.version.clone(),
            expires_at_unix_ms,
            items,
        };
        let mut batches = self
            .batches
            .lock()
            .map_err(|_| "Classification registry is unavailable".to_owned())?;
        batches.retain(|_, stored| stored.expires_at > now);
        if batches.len() >= 32 {
            return Err("Too many active classification batches".to_owned());
        }
        batches.insert(
            batch.batch_id.clone(),
            StoredBatch {
                expires_at: now + self.ttl,
                batch: batch.clone(),
            },
        );
        Ok(batch)
    }

    pub(crate) fn consume_at(
        &self,
        batch_id: &str,
        discovery_proposal_id: &str,
        item_ids: &[String],
        now: Instant,
    ) -> Result<ClassificationBatch, String> {
        validate_id(batch_id, "classification batch")?;
        validate_id(discovery_proposal_id, "discovery proposal")?;
        let expected = item_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        if expected.is_empty() || expected.len() != item_ids.len() {
            return Err("Classification selection is empty or duplicated".to_owned());
        }
        let mut batches = self
            .batches
            .lock()
            .map_err(|_| "Classification registry is unavailable".to_owned())?;
        batches.retain(|_, stored| stored.expires_at > now);
        let stored = batches
            .get(batch_id)
            .ok_or_else(|| "Unknown, expired, or consumed classification batch".to_owned())?;
        let actual = stored
            .batch
            .items
            .iter()
            .map(|item| item.item_id.as_str())
            .collect::<HashSet<_>>();
        if stored.batch.discovery_proposal_id != discovery_proposal_id || actual != expected {
            return Err("Classification batch does not match the reviewed selection".to_owned());
        }
        batches
            .remove(batch_id)
            .map(|stored| stored.batch)
            .ok_or_else(|| "Classification batch could not be consumed".to_owned())
    }
}

fn validate_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_OPAQUE_ID_BYTES
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err(format!("{label} ID is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ClassificationBatchRegistry, ClassificationItemInput};
    use crate::discovery::ReviewedSource;
    use crate::identity::ContentIdentity;
    use crate::profiles::proposal::{EvidenceReference, ProposalStatus};
    use crate::profiles::schema::{
        ClassificationRule, DeclarativeProfile, EvidenceKind, EvidenceRequirement,
        ProfileOwnership, ProfileProvenance, ProfileStatus,
    };
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime};

    fn identity() -> ContentIdentity {
        ContentIdentity {
            algorithm: "SHA-256".to_owned(),
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        }
    }

    fn profile(status: ProfileStatus) -> DeclarativeProfile {
        DeclarativeProfile {
            schema_version: 1,
            profile_id: "approved-profile".to_owned(),
            version: "1.0.0".to_owned(),
            title: "Approved profile".to_owned(),
            status,
            provenance: ProfileProvenance {
                source_title: "Owned fixture".to_owned(),
                ownership: ProfileOwnership::Owned,
                evidence: vec!["authorization:test".to_owned()],
            },
            categories: Vec::new(),
            governance: None,
            rules: vec![ClassificationRule {
                rule_id: "rule-report".to_owned(),
                destination: vec!["01-Research".to_owned(), "Reports".to_owned()],
                all_of: vec![EvidenceRequirement {
                    kind: EvidenceKind::DocumentText,
                    term: "quarterly report".to_owned(),
                }],
            }],
        }
    }

    fn source() -> ReviewedSource {
        ReviewedSource {
            item_id: "item-1".to_owned(),
            path: PathBuf::from("/review/report.pdf"),
            name: "report.pdf".to_owned(),
            byte_size: 42,
            identity: identity(),
        }
    }

    fn input(text: &str) -> ClassificationItemInput {
        ClassificationItemInput {
            item_id: "item-1".to_owned(),
            references: vec![EvidenceReference {
                kind: EvidenceKind::DocumentText,
                location: "page:1".to_owned(),
                text: text.to_owned(),
            }],
        }
    }

    #[test]
    fn binds_one_proposal_to_the_exact_approved_profile_and_reviewed_identity() {
        let registry = ClassificationBatchRegistry::default();
        let now = Instant::now();
        let batch = registry
            .create_at(
                "proposal-1",
                &profile(ProfileStatus::Approved),
                vec![source()],
                vec![input("Quarterly report for Atlas")],
                now,
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
            )
            .expect("create classification batch");

        assert_eq!(batch.profile_id, "approved-profile");
        assert_eq!(batch.profile_version, "1.0.0");
        assert_eq!(batch.items[0].proposal.status, ProposalStatus::Proposed);
        assert_eq!(
            batch.items[0].proposal.destination.as_deref(),
            Some(&["01-Research".to_owned(), "Reports".to_owned()][..])
        );
        assert_eq!(batch.items[0].proposal.source_identity, identity());
        assert!(batch.items[0].proposal.committable);
        assert!(registry
            .consume_at(&batch.batch_id, "proposal-1", &["item-1".to_owned()], now,)
            .is_ok());
        assert!(registry
            .consume_at(&batch.batch_id, "proposal-1", &["item-1".to_owned()], now,)
            .is_err());
    }

    #[test]
    fn rejects_draft_profiles_and_routes_missing_evidence_to_review() {
        let registry = ClassificationBatchRegistry::default();
        assert!(registry
            .create_at(
                "proposal-1",
                &profile(ProfileStatus::Draft),
                vec![source()],
                vec![input("Quarterly report")],
                Instant::now(),
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        let batch = registry
            .create_at(
                "proposal-1",
                &profile(ProfileStatus::Approved),
                vec![source()],
                vec![input("Unrelated evidence")],
                Instant::now(),
                SystemTime::UNIX_EPOCH,
            )
            .expect("create review batch");
        assert_eq!(
            batch.items[0].proposal.status,
            ProposalStatus::ClassificationReview
        );
        assert!(!batch.items[0].proposal.committable);
        assert!(batch.items[0].proposal.destination.is_none());
    }
}

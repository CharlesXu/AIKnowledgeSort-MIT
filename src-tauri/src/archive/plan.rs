use crate::discovery::ReviewedSource;
use crate::identity::ContentIdentity;
use crate::vault::VaultSummary;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PLAN_VERSION: u32 = 1;
const MAX_OPAQUE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlanItem {
    pub item_id: String,
    pub source_path: String,
    pub destination_path: String,
    pub byte_size: u64,
    pub identity: ContentIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePlan {
    pub plan_id: String,
    pub plan_version: u32,
    pub proposal_id: String,
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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "consumed by the archive transaction layer")
    )]
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

    pub(crate) fn create_at(
        &self,
        proposal_id: &str,
        sources: Vec<ReviewedSource>,
        vault: VaultSummary,
        now: Instant,
        wall_time: SystemTime,
    ) -> Result<ArchivePlan, String> {
        validate_opaque_id(proposal_id, "discovery proposal")?;
        if sources.is_empty() || sources.len() > self.limits.max_items {
            return Err("Archive plan selection is empty or too large".to_owned());
        }
        validate_opaque_id(&vault.authority_id, "Vault authority")?;

        let mut seen_item_ids = HashSet::with_capacity(sources.len());
        let mut items = Vec::with_capacity(sources.len());
        for source in sources {
            validate_opaque_id(&source.item_id, "reviewed item")?;
            if !seen_item_ids.insert(source.item_id.clone()) {
                return Err("Archive plan contains duplicate reviewed items".to_owned());
            }
            source.identity.validate()?;
            validate_original_name(&source.name)?;
            let destination = Path::new("Originals")
                .join(&source.identity.digest)
                .join(&source.name);
            items.push(ArchivePlanItem {
                item_id: source.item_id,
                source_path: source.path.to_string_lossy().into_owned(),
                destination_path: destination.to_string_lossy().into_owned(),
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

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "consumed by the archive transaction layer")
    )]
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

    #[test]
    fn creates_an_exact_source_preserving_plan() {
        let now = Instant::now();
        let registry = ArchivePlanRegistry::default();
        let plan = registry
            .create_at(
                "reviewed-proposal",
                vec![source("item-one", "report.md")],
                vault(),
                now,
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
            )
            .expect("create archive plan");

        assert_eq!(plan.plan_version, 1);
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
            .create_at(
                "reviewed-proposal",
                vec![source("item-one", "report.md")],
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
            .create_at(
                "reviewed-proposal",
                vec![source("item-two", "fresh.md")],
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
            .create_at(
                "reviewed-proposal",
                vec![duplicate.clone(), duplicate],
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(registry
            .create_at(
                "reviewed-proposal",
                vec![source("unsafe", "CON.txt")],
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        let mut invalid = source("invalid", "valid.md");
        invalid.identity.algorithm = "sha256".to_owned();
        assert!(registry
            .create_at(
                "reviewed-proposal",
                vec![invalid],
                vault(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
    }
}

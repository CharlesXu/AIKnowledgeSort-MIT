use crate::discovery::ReviewedSource;
use crate::naming::normalize::propose_name;
use crate::naming::schema::{canonical_policy, NamingFact, NamingProposal, NamingRequest};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_OPAQUE_ID_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NamingItemInput {
    pub item_id: String,
    pub facts: Vec<NamingFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamingBatch {
    pub batch_id: String,
    pub discovery_proposal_id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub expires_at_unix_ms: u64,
    pub proposals: Vec<NamingProposal>,
}

#[derive(Clone, Copy)]
pub(crate) struct NamingBatchRegistryLimits {
    pub max_batches: usize,
    pub max_items: usize,
    pub max_occupied_names: usize,
    pub ttl: Duration,
}

impl Default for NamingBatchRegistryLimits {
    fn default() -> Self {
        Self {
            max_batches: 32,
            max_items: 1_000,
            max_occupied_names: 10_000,
            ttl: Duration::from_secs(5 * 60),
        }
    }
}

struct StoredBatch {
    expires_at: Instant,
    batch: NamingBatch,
}

#[derive(Clone)]
pub struct NamingBatchRegistry {
    limits: NamingBatchRegistryLimits,
    batches: Arc<Mutex<HashMap<String, StoredBatch>>>,
}

impl Default for NamingBatchRegistry {
    fn default() -> Self {
        Self::new(NamingBatchRegistryLimits::default())
    }
}

impl NamingBatchRegistry {
    pub(crate) fn new(limits: NamingBatchRegistryLimits) -> Self {
        Self {
            limits,
            batches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn create_at(
        &self,
        discovery_proposal_id: &str,
        sources: Vec<ReviewedSource>,
        inputs: Vec<NamingItemInput>,
        occupied_names: HashMap<String, Vec<String>>,
        now: Instant,
        wall_time: SystemTime,
    ) -> Result<NamingBatch, String> {
        validate_opaque_id(discovery_proposal_id, "discovery proposal")?;
        if inputs.is_empty()
            || inputs.len() > self.limits.max_items
            || sources.len() != inputs.len()
        {
            return Err("Naming batch selection is empty, mismatched, or too large".to_owned());
        }

        let mut source_by_id = HashMap::with_capacity(sources.len());
        for source in sources {
            validate_opaque_id(&source.item_id, "reviewed item")?;
            source.identity.validate()?;
            if source_by_id
                .insert(source.item_id.clone(), source)
                .is_some()
            {
                return Err("Naming batch contains duplicate reviewed sources".to_owned());
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
            return Err("Naming inputs do not match the reviewed selection".to_owned());
        }
        if occupied_names
            .keys()
            .any(|item_id| !input_ids.contains(item_id.as_str()))
        {
            return Err("Occupied-name namespace contains an unknown item".to_owned());
        }
        let occupied_count = occupied_names
            .values()
            .try_fold(0_usize, |count, names| count.checked_add(names.len()))
            .ok_or_else(|| "Occupied-name count overflowed".to_owned())?;
        if occupied_count > self.limits.max_occupied_names {
            return Err("Occupied-name count exceeds the batch limit".to_owned());
        }

        let policy = canonical_policy();
        let mut proposals = Vec::with_capacity(inputs.len());
        for input in inputs {
            let source = source_by_id
                .remove(&input.item_id)
                .ok_or_else(|| "Naming input is not part of the reviewed selection".to_owned())?;
            proposals.push(propose_name(
                &policy,
                &NamingRequest {
                    item_id: source.item_id,
                    original_name: source.name,
                    identity: source.identity,
                    facts: input.facts,
                    occupied_names: occupied_names
                        .get(&input.item_id)
                        .cloned()
                        .unwrap_or_default(),
                },
            )?);
        }
        proposals.sort_by(|left, right| left.item_id.cmp(&right.item_id));

        let expires_at_unix_ms = wall_time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "System clock is before the Unix epoch".to_owned())?
            .checked_add(self.limits.ttl)
            .ok_or_else(|| "Naming batch expiry overflowed".to_owned())?
            .as_millis()
            .try_into()
            .map_err(|_| "Naming batch expiry is out of range".to_owned())?;

        let mut batches = self
            .batches
            .lock()
            .map_err(|_| "Naming batch registry is unavailable".to_owned())?;
        batches.retain(|_, stored| stored.expires_at > now);
        if batches.len() >= self.limits.max_batches {
            return Err("Too many active naming batches".to_owned());
        }
        let batch_id = unique_batch_id(&batches);
        let batch = NamingBatch {
            batch_id: batch_id.clone(),
            discovery_proposal_id: discovery_proposal_id.to_owned(),
            policy_id: policy.policy_id.to_owned(),
            policy_version: policy.version.to_owned(),
            expires_at_unix_ms,
            proposals,
        };
        batches.insert(
            batch_id,
            StoredBatch {
                expires_at: now + self.limits.ttl,
                batch: batch.clone(),
            },
        );
        Ok(batch)
    }

    pub fn consume_at(
        &self,
        batch_id: &str,
        discovery_proposal_id: &str,
        item_ids: &[String],
        now: Instant,
    ) -> Result<NamingBatch, String> {
        validate_opaque_id(batch_id, "naming batch")?;
        validate_opaque_id(discovery_proposal_id, "discovery proposal")?;
        if item_ids.is_empty() || item_ids.len() > self.limits.max_items {
            return Err("Naming batch selection is empty or too large".to_owned());
        }
        let requested = item_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        if requested.len() != item_ids.len() {
            return Err("Naming batch selection contains duplicate items".to_owned());
        }

        let mut batches = self
            .batches
            .lock()
            .map_err(|_| "Naming batch registry is unavailable".to_owned())?;
        batches.retain(|_, stored| stored.expires_at > now);
        let stored = batches
            .get(batch_id)
            .ok_or_else(|| "Unknown, expired, or already consumed naming batch".to_owned())?;
        let stored_items = stored
            .batch
            .proposals
            .iter()
            .map(|proposal| proposal.item_id.as_str())
            .collect::<HashSet<_>>();
        if stored.batch.discovery_proposal_id != discovery_proposal_id || stored_items != requested
        {
            return Err("Naming batch does not match the reviewed selection".to_owned());
        }
        batches
            .remove(batch_id)
            .map(|stored| stored.batch)
            .ok_or_else(|| "Naming batch could not be consumed".to_owned())
    }
}

fn validate_opaque_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_OPAQUE_ID_BYTES {
        Err(format!("Invalid {label} id"))
    } else {
        Ok(())
    }
}

fn unique_batch_id(batches: &HashMap<String, StoredBatch>) -> String {
    loop {
        let candidate = Uuid::new_v4().simple().to_string();
        if !batches.contains_key(&candidate) {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NamingBatchRegistry, NamingBatchRegistryLimits, NamingItemInput};
    use crate::discovery::ReviewedSource;
    use crate::identity::ContentIdentity;
    use crate::naming::schema::{NamingFact, NamingFactKind, NamingStatus};
    use std::collections::HashMap;
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
                digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_owned(),
            },
        }
    }

    fn item(item_id: &str, subject: Option<&str>) -> NamingItemInput {
        NamingItemInput {
            item_id: item_id.to_owned(),
            facts: subject
                .map(|value| {
                    vec![NamingFact {
                        kind: NamingFactKind::Subject,
                        value: value.to_owned(),
                        evidence_location: "page:1".to_owned(),
                    }]
                })
                .unwrap_or_else(|| {
                    vec![NamingFact {
                        kind: NamingFactKind::Project,
                        value: "Atlas".to_owned(),
                        evidence_location: "page:1".to_owned(),
                    }]
                }),
        }
    }

    #[test]
    fn binds_proposals_to_trusted_reviewed_sources_and_policy() {
        let registry = NamingBatchRegistry::default();
        let now = Instant::now();
        let batch = registry
            .create_at(
                "proposal-1",
                vec![
                    source("item-2", "untrusted-number-2.PDF"),
                    source("item-1", "untrusted-number-1.pdf"),
                ],
                vec![item("item-1", Some("Reset report")), item("item-2", None)],
                HashMap::from([("item-1".to_owned(), vec!["reset-report.PDF".to_owned()])]),
                now,
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
            )
            .expect("create naming batch");

        assert_eq!(batch.discovery_proposal_id, "proposal-1");
        assert_eq!(batch.policy_id, "canonical-v1");
        assert_eq!(batch.policy_version, "1.0.0");
        assert_eq!(batch.proposals[0].item_id, "item-1");
        assert_eq!(batch.proposals[0].original_name, "untrusted-number-1.pdf");
        assert_eq!(
            batch.proposals[0].canonical_name.as_deref(),
            Some("Reset-report--01234567.pdf")
        );
        assert_eq!(batch.proposals[0].status, NamingStatus::Proposed);
        assert_eq!(batch.proposals[1].status, NamingStatus::NamingReview);
    }

    #[test]
    fn consumes_only_one_exact_unexpired_batch() {
        let now = Instant::now();
        let registry = NamingBatchRegistry::new(NamingBatchRegistryLimits {
            max_batches: 2,
            max_items: 2,
            max_occupied_names: 4,
            ttl: Duration::from_secs(2),
        });
        let create = || {
            registry.create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![item("item-1", Some("Report"))],
                HashMap::new(),
                now,
                SystemTime::UNIX_EPOCH,
            )
        };

        let mismatched = create().expect("mismatch batch");
        assert!(registry
            .consume_at(
                &mismatched.batch_id,
                "proposal-2",
                &["item-1".to_owned()],
                now,
            )
            .is_err());
        assert!(registry
            .consume_at(
                &mismatched.batch_id,
                "proposal-1",
                &["item-2".to_owned()],
                now,
            )
            .is_err());
        assert!(registry
            .consume_at(
                &mismatched.batch_id,
                "proposal-1",
                &["item-1".to_owned()],
                now,
            )
            .is_ok());
        assert!(registry
            .consume_at(
                &mismatched.batch_id,
                "proposal-1",
                &["item-1".to_owned()],
                now,
            )
            .is_err());

        let expired = create().expect("expired batch");
        assert!(registry
            .consume_at(
                &expired.batch_id,
                "proposal-1",
                &["item-1".to_owned()],
                now + Duration::from_secs(3),
            )
            .is_err());
    }

    #[test]
    fn rejects_duplicate_mismatched_and_over_limit_inputs() {
        let now = Instant::now();
        let registry = NamingBatchRegistry::new(NamingBatchRegistryLimits {
            max_batches: 1,
            max_items: 1,
            max_occupied_names: 1,
            ttl: Duration::from_secs(60),
        });

        assert!(registry
            .create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![item("item-2", Some("Report"))],
                HashMap::new(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(registry
            .create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![
                    item("item-1", Some("Report")),
                    item("item-1", Some("Report")),
                ],
                HashMap::new(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(registry
            .create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![item("item-1", Some("Report"))],
                HashMap::from([(
                    "item-1".to_owned(),
                    vec!["one.pdf".to_owned(), "two.pdf".to_owned()],
                )]),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());

        registry
            .create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![item("item-1", Some("Report"))],
                HashMap::new(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .expect("first live batch");
        assert!(registry
            .create_at(
                "proposal-1",
                vec![source("item-1", "one.pdf")],
                vec![item("item-1", Some("Report"))],
                HashMap::new(),
                now,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
    }
}

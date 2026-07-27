use super::DiscoveryProposal;
use crate::identity::ContentIdentity;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_PROPOSAL_ID_BYTES: usize = 128;
const MAX_ITEM_ID_BYTES: usize = 128;

#[derive(Clone, Copy)]
pub(crate) struct ProposalRegistryLimits {
    pub max_proposals: usize,
    pub max_items: usize,
    pub ttl: Duration,
}

impl Default for ProposalRegistryLimits {
    fn default() -> Self {
        Self {
            max_proposals: 32,
            max_items: super::MAX_DISCOVERY_ITEMS,
            ttl: Duration::from_secs(15 * 60),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewedSource {
    pub item_id: String,
    pub path: PathBuf,
    pub name: String,
    pub byte_size: u64,
    pub identity: ContentIdentity,
}

struct ReviewedProposal {
    expires_at: Instant,
    items: HashMap<String, ReviewedSource>,
}

#[derive(Clone)]
pub(crate) struct ReviewedSourceRegistry {
    limits: ProposalRegistryLimits,
    proposals: Arc<Mutex<HashMap<String, ReviewedProposal>>>,
}

impl Default for ReviewedSourceRegistry {
    fn default() -> Self {
        Self::new(ProposalRegistryLimits::default())
    }
}

impl ReviewedSourceRegistry {
    pub(crate) fn new(limits: ProposalRegistryLimits) -> Self {
        Self {
            limits,
            proposals: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn register_at(
        &self,
        mut proposal: DiscoveryProposal,
        now: Instant,
    ) -> Result<DiscoveryProposal, String> {
        if !proposal.proposal_id.is_empty() {
            return Err("Discovery proposal is already registered".to_owned());
        }
        if proposal.items.len() > self.limits.max_items
            || proposal.counts.included != proposal.items.len()
        {
            return Err("Discovery proposal item count is invalid".to_owned());
        }

        let mut item_ids = HashSet::with_capacity(proposal.items.len());
        let mut reviewed_items = HashMap::with_capacity(proposal.items.len());
        for item in &proposal.items {
            if item.item_id.is_empty()
                || item.item_id.len() > MAX_ITEM_ID_BYTES
                || !item_ids.insert(item.item_id.clone())
            {
                return Err("Discovery proposal contains an invalid item id".to_owned());
            }
            item.identity.validate()?;
            reviewed_items.insert(
                item.item_id.clone(),
                ReviewedSource {
                    item_id: item.item_id.clone(),
                    path: PathBuf::from(&item.path),
                    name: item.name.clone(),
                    byte_size: item.byte_size,
                    identity: item.identity.clone(),
                },
            );
        }

        let mut proposals = self
            .proposals
            .lock()
            .map_err(|_| "Reviewed source registry is unavailable".to_owned())?;
        proposals.retain(|_, registered| registered.expires_at > now);
        if proposals.len() >= self.limits.max_proposals {
            return Err("Too many active discovery proposals".to_owned());
        }

        let proposal_id = unique_proposal_id(&proposals);
        proposal.proposal_id = proposal_id.clone();
        proposals.insert(
            proposal_id,
            ReviewedProposal {
                expires_at: now + self.limits.ttl,
                items: reviewed_items,
            },
        );
        Ok(proposal)
    }

    pub(crate) fn resolve_selection_at(
        &self,
        proposal_id: &str,
        item_ids: &[String],
        now: Instant,
    ) -> Result<Vec<ReviewedSource>, String> {
        if proposal_id.is_empty() || proposal_id.len() > MAX_PROPOSAL_ID_BYTES {
            return Err("Invalid discovery proposal id".to_owned());
        }
        if item_ids.is_empty() || item_ids.len() > self.limits.max_items {
            return Err("Archive selection is empty or too large".to_owned());
        }
        let unique_ids = item_ids.iter().collect::<HashSet<_>>();
        if unique_ids.len() != item_ids.len() {
            return Err("Archive selection contains duplicate items".to_owned());
        }

        let mut proposals = self
            .proposals
            .lock()
            .map_err(|_| "Reviewed source registry is unavailable".to_owned())?;
        proposals.retain(|_, registered| registered.expires_at > now);
        let proposal = proposals
            .get(proposal_id)
            .ok_or_else(|| "Unknown or expired discovery proposal".to_owned())?;

        item_ids
            .iter()
            .map(|item_id| {
                proposal
                    .items
                    .get(item_id)
                    .cloned()
                    .ok_or_else(|| "Selected item is not part of the reviewed proposal".to_owned())
            })
            .collect()
    }
}

fn unique_proposal_id(proposals: &HashMap<String, ReviewedProposal>) -> String {
    loop {
        let candidate = Uuid::new_v4().simple().to_string();
        if !proposals.contains_key(&candidate) {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProposalRegistryLimits, ReviewedSourceRegistry};
    use crate::discovery::{DiscoveredItem, DiscoveryCounts, DiscoveryProposal};
    use crate::identity::ContentIdentity;
    use std::time::{Duration, Instant};

    fn draft_proposal() -> DiscoveryProposal {
        DiscoveryProposal {
            proposal_id: String::new(),
            items: vec![
                DiscoveredItem {
                    item_id: "item-one".to_owned(),
                    path: "/review/one.txt".to_owned(),
                    name: "one.txt".to_owned(),
                    byte_size: 3,
                    identity: ContentIdentity {
                        algorithm: "SHA-256".to_owned(),
                        digest: "7692c3ad3540bb803c020b3aee66cd8887123234ea0c6e7143c0add73ff431ed"
                            .to_owned(),
                    },
                },
                DiscoveredItem {
                    item_id: "item-two".to_owned(),
                    path: "/review/two.txt".to_owned(),
                    name: "two.txt".to_owned(),
                    byte_size: 3,
                    identity: ContentIdentity {
                        algorithm: "SHA-256".to_owned(),
                        digest: "3fc4ccfe745870e2c0d99f71f30ff0656c8dedd41cc1d7d3d376b0dbe685e2f3"
                            .to_owned(),
                    },
                },
            ],
            counts: DiscoveryCounts {
                included: 2,
                ..DiscoveryCounts::default()
            },
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn registers_and_resolves_only_the_exact_reviewed_selection() {
        let now = Instant::now();
        let registry = ReviewedSourceRegistry::default();
        let proposal = registry
            .register_at(draft_proposal(), now)
            .expect("register reviewed proposal");

        assert!(!proposal.proposal_id.is_empty());
        let resolved = registry
            .resolve_selection_at(
                &proposal.proposal_id,
                &[proposal.items[1].item_id.clone()],
                now,
            )
            .expect("resolve selected item");

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].path.to_string_lossy(), "/review/two.txt");
        assert_eq!(resolved[0].identity, proposal.items[1].identity);
    }

    #[test]
    fn rejects_unknown_duplicate_and_expired_selections() {
        let now = Instant::now();
        let registry = ReviewedSourceRegistry::new(ProposalRegistryLimits {
            max_proposals: 2,
            max_items: 4,
            ttl: Duration::from_secs(5),
        });
        let proposal = registry
            .register_at(draft_proposal(), now)
            .expect("register reviewed proposal");
        let item_id = proposal.items[0].item_id.clone();

        assert!(registry
            .resolve_selection_at("unknown", std::slice::from_ref(&item_id), now)
            .is_err());
        assert!(registry
            .resolve_selection_at(
                &proposal.proposal_id,
                &[item_id.clone(), item_id.clone()],
                now,
            )
            .is_err());
        assert!(registry
            .resolve_selection_at(
                &proposal.proposal_id,
                &[item_id],
                now + Duration::from_secs(6),
            )
            .is_err());
    }
}

mod grant;
mod proposal;
mod walker;

use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::identity::ContentIdentity;

pub(crate) use grant::{
    issue_drop_grant, DropGrantRegistry, DROP_GRANT_ERROR_EVENT, DROP_GRANT_EVENT,
};
pub(crate) use proposal::ReviewedSourceRegistry;

const MAX_DISCOVERY_ITEMS: usize = 10_000;
const MAX_DIAGNOSTICS: usize = 256;
const MAX_DIRECTORY_DEPTH: usize = 64;
const MAX_PATH_BYTES: usize = 4_096;
pub(crate) const DROP_WORK_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BLOCKING_DROP_WORK: usize = 4;

#[derive(Clone)]
pub(crate) struct DropWorkLimiter {
    active: Arc<AtomicUsize>,
    max_active: usize,
}

impl Default for DropWorkLimiter {
    fn default() -> Self {
        Self::new(MAX_BLOCKING_DROP_WORK)
    }
}

impl DropWorkLimiter {
    pub(crate) fn new(max_active: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max_active,
        }
    }

    pub(crate) fn try_acquire(&self) -> Result<DropWorkPermit, String> {
        let mut active = self.active.load(Ordering::Acquire);
        loop {
            if active >= self.max_active {
                return Err("Too many filesystem discovery operations are active".to_owned());
            }
            match self.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(DropWorkPermit {
                        active: Arc::clone(&self.active),
                    })
                }
                Err(observed) => active = observed,
            }
        }
    }
}

pub(crate) struct DropWorkPermit {
    active: Arc<AtomicUsize>,
}

impl Drop for DropWorkPermit {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProposal {
    pub proposal_id: String,
    pub items: Vec<DiscoveredItem>,
    pub counts: DiscoveryCounts,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredItem {
    pub item_id: String,
    pub path: String,
    pub name: String,
    pub byte_size: u64,
    pub identity: ContentIdentity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCounts {
    pub included: usize,
    pub excluded: usize,
    pub unreadable: usize,
    pub symlink: usize,
    pub out_of_scope: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDiagnostic {
    pub category: DiagnosticCategory,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticCategory {
    Excluded,
    Unreadable,
    Symlink,
    TraversalLimit,
}

#[tauri::command]
pub async fn propose_local_drop(
    grant_id: String,
    registry: tauri::State<'_, DropGrantRegistry>,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    limiter: tauri::State<'_, DropWorkLimiter>,
) -> Result<DiscoveryProposal, String> {
    let permit = limiter.try_acquire()?;
    let grant = registry.consume_at(&grant_id, Instant::now())?;
    let reviewed_sources = reviewed_sources.inner().clone();
    let deadline = Instant::now() + DROP_WORK_TIMEOUT;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let proposal = walker::discover_grant_with_deadline(grant, MAX_DISCOVERY_ITEMS, deadline)?;
        let registered_at = Instant::now();
        let proposal = reviewed_sources.register_at(proposal, registered_at)?;
        if !proposal.items.is_empty() {
            let item_ids = proposal
                .items
                .iter()
                .map(|item| item.item_id.clone())
                .collect::<Vec<_>>();
            let resolved = reviewed_sources.resolve_selection_at(
                &proposal.proposal_id,
                &item_ids,
                registered_at,
            )?;
            if resolved.len() != proposal.items.len() {
                return Err("Reviewed source registration is incomplete".to_owned());
            }
        }
        Ok(proposal)
    });

    match tokio::time::timeout(DROP_WORK_TIMEOUT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(bounded_error(format!(
            "Filesystem discovery worker failed: {error}"
        ))),
        Err(_) => Err("Filesystem discovery deadline exceeded".to_owned()),
    }
}

pub(crate) fn bounded_error(error: String) -> String {
    error.chars().take(512).collect()
}

#[cfg(test)]
mod tests;

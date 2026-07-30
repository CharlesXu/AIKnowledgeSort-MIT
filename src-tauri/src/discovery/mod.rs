mod grant;
mod proposal;
mod walker;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri_plugin_dialog::FilePath;

use crate::identity::ContentIdentity;

pub(crate) use grant::{
    issue_drop_grant, open_trusted_drop_root, CapabilityRoot, DropGrantRegistry,
    DROP_GRANT_ERROR_EVENT, DROP_GRANT_EVENT,
};
pub(crate) use proposal::{ReviewedSource, ReviewedSourceRegistry};

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
pub async fn choose_local_files(
    app: tauri::AppHandle,
    registry: tauri::State<'_, DropGrantRegistry>,
    limiter: tauri::State<'_, DropWorkLimiter>,
) -> Result<Option<grant::DropGrantIssued>, String> {
    let Some(selection) = crate::native_dialog::pick_files(&app).await? else {
        return Ok(None);
    };
    let paths = selected_local_paths(selection)?;
    issue_local_source_grant(paths, registry.inner().clone(), limiter.inner().clone())
        .await
        .map(Some)
}

#[tauri::command]
pub async fn choose_local_folders(
    app: tauri::AppHandle,
    registry: tauri::State<'_, DropGrantRegistry>,
    limiter: tauri::State<'_, DropWorkLimiter>,
) -> Result<Option<grant::DropGrantIssued>, String> {
    let Some(selection) = crate::native_dialog::pick_folders(&app).await? else {
        return Ok(None);
    };
    let paths = selected_local_paths(selection)?;
    issue_local_source_grant(paths, registry.inner().clone(), limiter.inner().clone())
        .await
        .map(Some)
}

fn selected_local_paths(selection: Vec<FilePath>) -> Result<Vec<PathBuf>, String> {
    selection
        .into_iter()
        .map(|selected| {
            selected
                .into_path()
                .map_err(|_| "Selected source is not a local filesystem path".to_owned())
        })
        .collect()
}

pub(crate) async fn issue_local_source_grant(
    paths: Vec<PathBuf>,
    registry: DropGrantRegistry,
    limiter: DropWorkLimiter,
) -> Result<grant::DropGrantIssued, String> {
    let permit = limiter.try_acquire()?;
    let deadline = Instant::now() + DROP_WORK_TIMEOUT;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        issue_drop_grant(&registry, paths, deadline)
    });

    match tokio::time::timeout(DROP_WORK_TIMEOUT, task).await {
        Ok(Ok(result)) => result.map_err(bounded_error),
        Ok(Err(error)) => Err(bounded_error(format!(
            "Source grant worker failed: {error}"
        ))),
        Err(_) => Err("Source grant processing deadline exceeded".to_owned()),
    }
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

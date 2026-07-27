mod grant;
mod walker;

use serde::Serialize;
use std::time::Instant;

pub(crate) use grant::{
    issue_drop_grant, DropGrantRegistry, DROP_GRANT_ERROR_EVENT, DROP_GRANT_EVENT,
};

const MAX_DISCOVERY_ITEMS: usize = 10_000;
const MAX_DIAGNOSTICS: usize = 256;
const MAX_DIRECTORY_DEPTH: usize = 64;
const MAX_PATH_BYTES: usize = 4_096;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProposal {
    pub items: Vec<DiscoveredItem>,
    pub counts: DiscoveryCounts,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredItem {
    pub path: String,
    pub name: String,
    pub byte_size: u64,
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
pub fn propose_local_drop(
    grant_id: String,
    registry: tauri::State<'_, DropGrantRegistry>,
) -> Result<DiscoveryProposal, String> {
    let grant = registry.consume_at(&grant_id, Instant::now())?;
    walker::discover_grant_with_limit(grant, MAX_DISCOVERY_ITEMS)
}

#[cfg(test)]
mod tests;

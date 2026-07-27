mod plan;

use crate::discovery::ReviewedSourceRegistry;
use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::time::{Instant, SystemTime};

pub use plan::{ArchivePlan, ArchivePlanRegistry};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArchivePlanRequest {
    proposal_id: String,
    item_ids: Vec<String>,
}

#[tauri::command]
pub fn create_archive_plan(
    request: CreateArchivePlanRequest,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchivePlanRegistry>,
) -> Result<ArchivePlan, String> {
    let now = Instant::now();
    let sources =
        reviewed_sources.resolve_selection_at(&request.proposal_id, &request.item_ids, now)?;
    let vault = vaults.current_summary()?;
    plans.create_at(&request.proposal_id, sources, vault, now, SystemTime::now())
}

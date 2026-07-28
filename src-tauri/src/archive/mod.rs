mod plan;
mod transaction;

use crate::discovery::ReviewedSourceRegistry;
use crate::naming::NamingBatchRegistry;
use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::time::{Instant, SystemTime};

pub use plan::{ArchivePlan, ArchivePlanRegistry};
pub(crate) use transaction::reconcile_vault;
pub use transaction::ArchiveCommitResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateArchivePlanRequest {
    proposal_id: String,
    item_ids: Vec<String>,
    naming_batch_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmArchivePlanRequest {
    plan_id: String,
    confirmation_nonce: String,
}

#[tauri::command]
pub fn create_archive_plan(
    request: CreateArchivePlanRequest,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    naming_batches: tauri::State<'_, NamingBatchRegistry>,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchivePlanRegistry>,
) -> Result<ArchivePlan, String> {
    let now = Instant::now();
    let sources =
        reviewed_sources.resolve_selection_at(&request.proposal_id, &request.item_ids, now)?;
    let vault = vaults.current_summary()?;
    let naming_batch = naming_batches.consume_at(
        &request.naming_batch_id,
        &request.proposal_id,
        &request.item_ids,
        now,
    )?;
    plans.create_named_at(
        &request.proposal_id,
        sources,
        naming_batch,
        vault,
        now,
        SystemTime::now(),
    )
}

#[tauri::command]
pub async fn confirm_archive_plan(
    request: ConfirmArchivePlanRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchivePlanRegistry>,
) -> Result<ArchiveCommitResult, String> {
    let plan = plans.consume_at(
        &request.plan_id,
        &request.confirmation_nonce,
        Instant::now(),
    )?;
    let vault = vaults.lease(&plan.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        transaction::commit_plan_with_faults(plan, &vault, Default::default())
    })
    .await
    .map_err(|error| format!("Archive transaction worker failed: {error}"))
}

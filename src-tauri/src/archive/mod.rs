mod plan;
mod transaction;
pub(crate) mod undo;

use crate::discovery::ReviewedSourceRegistry;
use crate::naming::NamingBatchRegistry;
use crate::profiles::ClassificationBatchRegistry;
use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::time::{Instant, SystemTime};

#[cfg(test)]
pub(crate) use plan::ArchivePlanItem;
pub use plan::{ArchivePlan, ArchivePlanRegistry};
pub use transaction::ArchiveCommitResult;
#[cfg(test)]
pub(crate) use transaction::{commit_plan_with_faults, TransactionFaults};
pub(crate) use transaction::{
    list_verified_registered_originals, reconcile_vault, verified_registered_original,
};
pub(crate) use undo::reconcile_vault as reconcile_undo_vault;
pub use undo::ArchiveUndoPlanRegistry;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateArchivePlanRequest {
    proposal_id: String,
    item_ids: Vec<String>,
    classification_batch_id: String,
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
    classification_batches: tauri::State<'_, ClassificationBatchRegistry>,
    naming_batches: tauri::State<'_, NamingBatchRegistry>,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchivePlanRegistry>,
) -> Result<ArchivePlan, String> {
    let now = Instant::now();
    let sources =
        reviewed_sources.resolve_selection_at(&request.proposal_id, &request.item_ids, now)?;
    let vault = vaults.current_summary()?;
    let classification_batch = classification_batches.consume_at(
        &request.classification_batch_id,
        &request.proposal_id,
        &request.item_ids,
        now,
    )?;
    let naming_batch = naming_batches.consume_at(
        &request.naming_batch_id,
        &request.proposal_id,
        &request.item_ids,
        now,
    )?;
    plans.create_classified_named_at(
        &request.proposal_id,
        sources,
        classification_batch,
        naming_batch,
        vault,
        plan::PlanClock {
            monotonic: now,
            wall: SystemTime::now(),
        },
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

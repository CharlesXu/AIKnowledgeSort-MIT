pub mod normalize;
mod registry;
pub mod schema;

use crate::discovery::ReviewedSourceRegistry;
use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Instant, SystemTime};

pub use registry::{NamingBatch, NamingBatchRegistry, NamingItemInput};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNamingBatchRequest {
    proposal_id: String,
    items: Vec<NamingItemInput>,
}

#[tauri::command]
pub async fn create_naming_batch(
    request: CreateNamingBatchRequest,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    batches: tauri::State<'_, NamingBatchRegistry>,
) -> Result<NamingBatch, String> {
    let reviewed_sources = reviewed_sources.inner().clone();
    let vaults = vaults.inner().clone();
    let batches = batches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let now = Instant::now();
        let item_ids = request
            .items
            .iter()
            .map(|item| item.item_id.clone())
            .collect::<Vec<_>>();
        let sources =
            reviewed_sources.resolve_selection_at(&request.proposal_id, &item_ids, now)?;
        let summary = vaults.current_summary()?;
        let vault = vaults.lease(&summary.authority_id)?;

        let mut names_by_digest = HashMap::<String, Vec<String>>::new();
        for source in &sources {
            if !names_by_digest.contains_key(&source.identity.digest) {
                let names = vault.occupied_names_for_digest(
                    &source.identity.digest,
                    schema::MAX_OCCUPIED_NAMES,
                )?;
                names_by_digest.insert(source.identity.digest.clone(), names);
            }
        }
        let occupied_names = sources
            .iter()
            .map(|source| {
                (
                    source.item_id.clone(),
                    names_by_digest
                        .get(&source.identity.digest)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect();

        batches.create_at(
            &request.proposal_id,
            sources,
            request.items,
            occupied_names,
            now,
            SystemTime::now(),
        )
    })
    .await
    .map_err(|error| format!("Naming batch worker failed: {error}"))?
}

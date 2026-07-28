mod store;

use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;

pub use store::KnowledgeDocument;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenKnowledgeDocumentRequest {
    authority_id: String,
    operation_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveKnowledgeDocumentRequest {
    authority_id: String,
    operation_id: String,
    expected_revision: u32,
    markdown: String,
}

#[tauri::command]
pub async fn open_knowledge_document(
    request: OpenKnowledgeDocumentRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<KnowledgeDocument, String> {
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        store::open_document(&vault, &request.operation_id)
    })
    .await
    .map_err(|error| format!("Knowledge open worker failed: {error}"))?
}

#[tauri::command]
pub async fn save_knowledge_document(
    request: SaveKnowledgeDocumentRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<KnowledgeDocument, String> {
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        store::save_document(
            &vault,
            &request.operation_id,
            request.expected_revision,
            &request.markdown,
        )
    })
    .await
    .map_err(|error| format!("Knowledge save worker failed: {error}"))?
}

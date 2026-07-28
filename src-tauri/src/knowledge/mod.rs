mod store;

use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub use store::KnowledgeDocument;
pub(crate) use store::open_committed_revision;
#[cfg(test)]
pub(crate) use store::save_document;

#[derive(Clone, Default)]
pub struct KnowledgeWriteRegistry {
    active: Arc<Mutex<HashSet<String>>>,
}

struct KnowledgeWritePermit {
    key: String,
    active: Arc<Mutex<HashSet<String>>>,
}

impl KnowledgeWriteRegistry {
    fn acquire(
        &self,
        authority_id: &str,
        operation_id: &str,
    ) -> Result<KnowledgeWritePermit, String> {
        let key = format!("{authority_id}:{operation_id}");
        let mut active = self
            .active
            .lock()
            .map_err(|_| "Knowledge write registry is unavailable".to_owned())?;
        if !active.insert(key.clone()) {
            return Err("Knowledge document already has a save in progress".to_owned());
        }
        Ok(KnowledgeWritePermit {
            key,
            active: Arc::clone(&self.active),
        })
    }
}

impl Drop for KnowledgeWritePermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

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
    writes: tauri::State<'_, KnowledgeWriteRegistry>,
) -> Result<KnowledgeDocument, String> {
    let permit = writes.acquire(&request.authority_id, &request.operation_id)?;
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
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

#[cfg(test)]
mod tests {
    use super::KnowledgeWriteRegistry;

    #[test]
    fn serializes_writes_for_one_exact_vault_document() {
        let writes = KnowledgeWriteRegistry::default();
        let first = writes.acquire("vault", "operation").expect("first permit");
        assert!(writes.acquire("vault", "operation").is_err());
        assert!(writes.acquire("vault", "other-operation").is_ok());
        drop(first);
        assert!(writes.acquire("vault", "operation").is_ok());
    }
}

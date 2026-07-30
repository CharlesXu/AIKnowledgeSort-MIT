mod store;

use crate::vault::VaultAuthorityRegistry;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub use store::{
    EvidenceRange, GraphDecision, GraphRelation, GraphSnapshot, RelationRevisionInput,
};

#[derive(Clone, Default)]
pub struct GraphWriteRegistry {
    active: Arc<Mutex<HashSet<String>>>,
}

struct GraphWritePermit {
    key: String,
    active: Arc<Mutex<HashSet<String>>>,
}

impl GraphWriteRegistry {
    fn acquire(&self, key: String) -> Result<GraphWritePermit, String> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| "Graph write registry is unavailable".to_owned())?;
        if !active.insert(key.clone()) {
            return Err("Graph target already has a write in progress".to_owned());
        }
        Ok(GraphWritePermit {
            key,
            active: Arc::clone(&self.active),
        })
    }
}

impl Drop for GraphWritePermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectKnowledgeGraphRequest {
    authority_id: String,
    operation_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposeGraphRelationRequest {
    authority_id: String,
    operation_id: String,
    knowledge_revision: u32,
    source_node: String,
    relation_type: String,
    target_node: String,
    evidence_ranges: Vec<EvidenceRange>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportComparisonRelationsRequest {
    authority_id: String,
    comparison_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecideGraphRelationRequest {
    authority_id: String,
    relation_id: String,
    expected_version: u32,
    decision: GraphDecision,
    reason: String,
    revision: Option<RelationRevisionInput>,
}

#[tauri::command]
pub async fn inspect_knowledge_graph(
    request: InspectKnowledgeGraphRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<GraphSnapshot, String> {
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        store::inspect_graph(&vault, &request.operation_id)
    })
    .await
    .map_err(|error| format!("Graph inspection worker failed: {error}"))?
}

#[tauri::command]
pub async fn propose_graph_relation(
    request: ProposeGraphRelationRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    writes: tauri::State<'_, GraphWriteRegistry>,
) -> Result<GraphRelation, String> {
    let permit = writes.acquire(format!(
        "{}:document:{}",
        request.authority_id, request.operation_id
    ))?;
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        store::propose_relation(
            &vault,
            &request.operation_id,
            request.knowledge_revision,
            &request.source_node,
            &request.relation_type,
            &request.target_node,
            &request.evidence_ranges,
        )
    })
    .await
    .map_err(|error| format!("Graph proposal worker failed: {error}"))?
}

#[tauri::command]
pub async fn import_comparison_relations(
    request: ImportComparisonRelationsRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    writes: tauri::State<'_, GraphWriteRegistry>,
) -> Result<Vec<GraphRelation>, String> {
    let permit = writes.acquire(format!(
        "{}:comparison:{}",
        request.authority_id, request.comparison_id
    ))?;
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        store::import_comparison_relations(&vault, &request.comparison_id)
    })
    .await
    .map_err(|error| format!("Graph comparison import worker failed: {error}"))?
}

#[tauri::command]
pub async fn decide_graph_relation(
    request: DecideGraphRelationRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    writes: tauri::State<'_, GraphWriteRegistry>,
) -> Result<GraphRelation, String> {
    let permit = writes.acquire(format!(
        "{}:relation:{}",
        request.authority_id, request.relation_id
    ))?;
    let vault = vaults.lease(&request.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        store::decide_relation(
            &vault,
            &request.relation_id,
            request.expected_version,
            request.decision,
            &request.reason,
            request.revision.as_ref(),
        )
    })
    .await
    .map_err(|error| format!("Graph decision worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::GraphWriteRegistry;

    #[test]
    fn serializes_one_exact_graph_write_target() {
        let registry = GraphWriteRegistry::default();
        let first = registry.acquire("vault:relation:one".to_owned()).unwrap();
        assert!(registry.acquire("vault:relation:one".to_owned()).is_err());
        assert!(registry.acquire("vault:relation:two".to_owned()).is_ok());
        drop(first);
        assert!(registry.acquire("vault:relation:one".to_owned()).is_ok());
    }
}

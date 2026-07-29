mod config;
mod openai_compatible;
mod protocol;
mod store;

use config::{ModelConfigInput, ModelConfigStore, ModelConfigSummary, ModelRuntimeState};
use openai_compatible::OpenAiCompatibleTransport;
pub(crate) use protocol::{
    AgentAdjudication, AgentDecision, ComparisonRecord, ComparisonStatus, ModelProposal,
    ProviderOutcome,
};
#[cfg(test)]
pub(crate) use protocol::{ProposalSide, RelationSuggestion};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) use store::{
    build_comparison_envelope, persist_comparison_record, EvidenceRange, PreparedComparison,
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct ModelRuntimeAuthority {
    operation: Arc<Mutex<()>>,
    active_comparisons: Arc<Mutex<HashSet<String>>>,
}

struct ComparisonPermit {
    key: String,
    active: Arc<Mutex<HashSet<String>>>,
}

impl Drop for ComparisonPermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

impl ModelRuntimeAuthority {
    fn inspect(&self, directory: PathBuf) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).inspect()
    }

    fn upsert(
        &self,
        directory: PathBuf,
        request: ModelConfigInput,
    ) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).upsert(request)
    }

    fn remove(&self, directory: PathBuf, config_id: &str) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).remove(config_id)
    }

    fn load_pair(
        &self,
        directory: PathBuf,
        desktop_config_id: &str,
        agent_config_id: &str,
    ) -> Result<(ModelConfigSummary, ModelConfigSummary), String> {
        if desktop_config_id == agent_config_id {
            return Err("Desktop and Agent model configurations must be distinct".to_owned());
        }
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        let store = ModelConfigStore::new(directory);
        Ok((store.get(desktop_config_id)?, store.get(agent_config_id)?))
    }

    fn acquire_comparison(
        &self,
        authority_id: &str,
        operation_id: &str,
    ) -> Result<ComparisonPermit, String> {
        let key = format!("{authority_id}:{operation_id}");
        let mut active = self
            .active_comparisons
            .lock()
            .map_err(|_| "Model comparison registry is unavailable".to_owned())?;
        if !active.insert(key.clone()) {
            return Err("This knowledge document already has a comparison in progress".to_owned());
        }
        Ok(ComparisonPermit {
            key,
            active: Arc::clone(&self.active_comparisons),
        })
    }
}

pub(crate) trait ModelTransport: Sync {
    fn propose(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
    ) -> Result<ModelProposal, String>;

    fn adjudicate(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
        desktop: &ModelProposal,
        agent: &ModelProposal,
    ) -> Result<AgentAdjudication, String>;
}

pub(crate) fn run_comparison_with_transport(
    vault: &crate::vault::VaultLease,
    operation_id: &str,
    knowledge_revision: u32,
    evidence_ranges: &[EvidenceRange],
    desktop_config: &ModelConfigSummary,
    agent_config: &ModelConfigSummary,
    transport: &dyn ModelTransport,
) -> Result<ComparisonRecord, String> {
    if desktop_config.config_id == agent_config.config_id {
        return Err("Desktop and Agent model configurations must be distinct".to_owned());
    }
    let prepared =
        build_comparison_envelope(vault, operation_id, knowledge_revision, evidence_ranges)?;
    let (desktop_result, agent_result) =
        run_independent_proposals(transport, desktop_config, agent_config, &prepared);
    let desktop_outcome = proposal_outcome(desktop_config, desktop_result, &prepared);
    let agent_outcome = proposal_outcome(agent_config, agent_result, &prepared);

    let (adjudication, adjudication_failure, status) = match (
        desktop_outcome.proposal.as_ref(),
        agent_outcome.proposal.as_ref(),
    ) {
        (Some(desktop), Some(agent)) => match transport
            .adjudicate(agent_config, &prepared.json, desktop, agent)
            .and_then(|decision| {
                decision.validate(&prepared.envelope)?;
                Ok(decision)
            }) {
            Ok(decision) => {
                let status = if decision.decision == AgentDecision::Review {
                    ComparisonStatus::Review
                } else {
                    ComparisonStatus::Completed
                };
                (Some(decision), None, status)
            }
            Err(error) => (None, Some(bounded_failure(error)), ComparisonStatus::Review),
        },
        _ => (None, None, ComparisonStatus::Failed),
    };
    let record = ComparisonRecord {
        schema_version: 1,
        comparison_id: Uuid::new_v4().simple().to_string(),
        envelope: prepared.envelope,
        envelope_identity: prepared.identity,
        desktop_config_id: desktop_config.config_id.clone(),
        agent_config_id: agent_config.config_id.clone(),
        desktop_outcome,
        agent_outcome,
        adjudication,
        adjudication_failure,
        status,
        actor: "desktop-orchestrator".to_owned(),
        recorded_at_unix_ms: unix_time_ms(),
    };
    persist_comparison_record(vault, &record)?;
    Ok(record)
}

fn run_independent_proposals(
    transport: &dyn ModelTransport,
    desktop_config: &ModelConfigSummary,
    agent_config: &ModelConfigSummary,
    prepared: &PreparedComparison,
) -> (Result<ModelProposal, String>, Result<ModelProposal, String>) {
    std::thread::scope(|scope| {
        let desktop = scope.spawn(|| transport.propose(desktop_config, &prepared.json));
        let agent = scope.spawn(|| transport.propose(agent_config, &prepared.json));
        (
            desktop
                .join()
                .unwrap_or_else(|_| Err("Desktop model worker failed".to_owned())),
            agent
                .join()
                .unwrap_or_else(|_| Err("Agent model worker failed".to_owned())),
        )
    })
}

fn proposal_outcome(
    config: &ModelConfigSummary,
    result: Result<ModelProposal, String>,
    prepared: &PreparedComparison,
) -> ProviderOutcome {
    match result.and_then(|proposal| {
        proposal.validate(&prepared.envelope)?;
        Ok(proposal)
    }) {
        Ok(proposal) => ProviderOutcome::succeeded(config.model.clone(), proposal),
        Err(error) => ProviderOutcome::failed(bounded_failure(error)),
    }
}

fn bounded_failure(error: String) -> String {
    let normalized = error
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(2_048)
        .collect::<String>();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        "Model runtime failed".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveModelConfigRequest {
    config_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunModelComparisonRequest {
    authority_id: String,
    operation_id: String,
    knowledge_revision: u32,
    evidence_ranges: Vec<EvidenceRange>,
    desktop_config_id: String,
    agent_config_id: String,
}

fn app_config_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Application configuration directory is unavailable: {error}"))
}

#[tauri::command]
pub fn inspect_model_runtime(
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.inspect(app_config_directory(&app)?)
}

#[tauri::command]
pub fn upsert_model_config(
    request: ModelConfigInput,
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.upsert(app_config_directory(&app)?, request)
}

#[tauri::command]
pub fn remove_model_config(
    request: RemoveModelConfigRequest,
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.remove(app_config_directory(&app)?, &request.config_id)
}

#[tauri::command]
pub async fn run_model_comparison(
    request: RunModelComparisonRequest,
    app: tauri::AppHandle,
    vaults: tauri::State<'_, crate::vault::VaultAuthorityRegistry>,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ComparisonRecord, String> {
    let permit = authority.acquire_comparison(&request.authority_id, &request.operation_id)?;
    let vault = vaults.lease(&request.authority_id)?;
    let (desktop_config, agent_config) = authority.load_pair(
        app_config_directory(&app)?,
        &request.desktop_config_id,
        &request.agent_config_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        run_comparison_with_transport(
            &vault,
            &request.operation_id,
            request.knowledge_revision,
            &request.evidence_ranges,
            &desktop_config,
            &agent_config,
            &OpenAiCompatibleTransport,
        )
    })
    .await
    .map_err(|error| format!("Model comparison worker failed: {error}"))?
}

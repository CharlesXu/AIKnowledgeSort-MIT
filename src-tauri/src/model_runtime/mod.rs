mod config;
mod credential;
pub(crate) mod file_semantics;
mod model_discovery;
mod openai_compatible;
mod protocol;
mod store;

use config::{credential_environment, ModelConfigInput, ModelConfigStore, ModelRuntimeState};
pub(crate) use config::{ModelConfigSummary, ModelCredentialSource};
#[cfg(test)]
pub(crate) use config::{ModelLocation, ModelProtocol};
use credential::{CredentialVault, SystemCredentialVault};
pub(crate) use file_semantics::FileSemanticComparison;
use model_discovery::{DiscoverModelsRequest, DiscoveredModels};
pub(crate) use openai_compatible::complete_json;
use openai_compatible::{OpenAiCompatibleTransport, OpenAiFileSemanticTransport};
pub(crate) use protocol::{
    AgentAdjudication, AgentDecision, ComparisonRecord, ComparisonStatus, EvidenceExcerpt,
    ModelProposal, ProposalSide, ProviderOutcome, RelationSuggestion,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
pub(crate) use store::{
    build_comparison_envelope, load_comparison_record, persist_comparison_record, EvidenceRange,
    PreparedComparison,
};
use tauri::Manager;
use uuid::Uuid;

#[derive(Clone)]
pub struct ModelRuntimeAuthority {
    operation: Arc<Mutex<()>>,
    active_comparisons: Arc<Mutex<HashSet<String>>>,
    credentials: Arc<dyn CredentialVault>,
}

impl Default for ModelRuntimeAuthority {
    fn default() -> Self {
        Self {
            operation: Arc::new(Mutex::new(())),
            active_comparisons: Arc::new(Mutex::new(HashSet::new())),
            credentials: Arc::new(SystemCredentialVault),
        }
    }
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
    #[cfg(test)]
    fn with_credentials(credentials: Arc<dyn CredentialVault>) -> Self {
        Self {
            operation: Arc::new(Mutex::new(())),
            active_comparisons: Arc::new(Mutex::new(HashSet::new())),
            credentials,
        }
    }

    fn inspect(&self, directory: PathBuf) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        self.hydrate_state(ModelConfigStore::new(directory).inspect()?)
    }

    fn upsert(
        &self,
        directory: PathBuf,
        mut request: ModelConfigInput,
    ) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        request.endpoint_url = model_discovery::normalize_completion_endpoint(
            &request.endpoint_url,
            request.provider_protocol,
        )?;
        let store = ModelConfigStore::new(directory);
        let previous = store.get_input(&request.config_id).ok();
        let uses_keychain =
            request.authenticated && request.credential_source == ModelCredentialSource::Keychain;
        let previously_used_keychain = previous.as_ref().is_some_and(|config| {
            config.authenticated && config.credential_source == ModelCredentialSource::Keychain
        });
        let previous_credential = if uses_keychain || previously_used_keychain {
            self.credentials.read(&request.config_id)?
        } else {
            None
        };
        if uses_keychain {
            if let Some(api_key) = request.api_key.as_deref() {
                config::validate_api_key(api_key)?;
                self.credentials.set(&request.config_id, api_key)?;
            } else if previous_credential.is_none() {
                return Err("Enter an API key for the system credential vault".to_owned());
            }
        }
        let result = store.upsert(request.clone());
        let state = match result {
            Ok(state) => state,
            Err(error) => {
                if uses_keychain && request.api_key.is_some() {
                    if let Some(value) = previous_credential.as_deref() {
                        let _ = self.credentials.set(&request.config_id, value);
                    } else {
                        let _ = self.credentials.delete(&request.config_id);
                    }
                }
                return Err(error);
            }
        };
        if previously_used_keychain && !uses_keychain {
            self.credentials.delete(&request.config_id)?;
        }
        self.hydrate_state(state)
    }

    fn remove(&self, directory: PathBuf, config_id: &str) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        let store = ModelConfigStore::new(directory);
        let previous = store.get_input(config_id)?;
        let state = store.remove(config_id)?;
        if previous.authenticated && previous.credential_source == ModelCredentialSource::Keychain {
            self.credentials.delete(config_id)?;
        }
        self.hydrate_state(state)
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
        Ok((
            self.hydrate_config(store.get(desktop_config_id)?)?,
            self.hydrate_config(store.get(agent_config_id)?)?,
        ))
    }

    pub(crate) fn load_config(
        &self,
        directory: PathBuf,
        config_id: &str,
    ) -> Result<ModelConfigSummary, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        self.hydrate_config(ModelConfigStore::new(directory).get(config_id)?)
    }

    fn discover(&self, request: DiscoverModelsRequest) -> Result<DiscoveredModels, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        let credential = self.discovery_credential(&request)?;
        model_discovery::discover(&request, credential.as_deref())
    }

    fn discovery_credential(
        &self,
        request: &DiscoverModelsRequest,
    ) -> Result<Option<String>, String> {
        if !request.authenticated {
            return Ok(None);
        }
        if let Some(api_key) = request.api_key.as_deref() {
            config::validate_api_key(api_key)?;
            return Ok(Some(api_key.to_owned()));
        }
        let config_id = request
            .config_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Enter a configuration ID or API key for discovery".to_owned())?;
        match request.credential_source {
            ModelCredentialSource::Environment => {
                let environment = credential_environment(config_id);
                std::env::var(&environment)
                    .map(Some)
                    .map_err(|_| format!("Model credential environment {environment} is not set"))
            }
            ModelCredentialSource::Keychain => self.credentials.read(config_id)?.map_or_else(
                || Err("No API key is stored for this model configuration".to_owned()),
                |value| Ok(Some(value)),
            ),
        }
    }

    fn hydrate_state(&self, mut state: ModelRuntimeState) -> Result<ModelRuntimeState, String> {
        state.configs = state
            .configs
            .into_iter()
            .map(|config| self.hydrate_config(config))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(state)
    }

    fn hydrate_config(&self, mut config: ModelConfigSummary) -> Result<ModelConfigSummary, String> {
        if config.authenticated && config.credential_source == ModelCredentialSource::Keychain {
            config.credential_value = self.credentials.read(&config.config_id)?;
            config.credential_stored = config.credential_value.is_some();
        }
        Ok(config)
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
            return Err("This semantic target already has a comparison in progress".to_owned());
        }
        Ok(ComparisonPermit {
            key,
            active: Arc::clone(&self.active_comparisons),
        })
    }
}

#[cfg(test)]
mod credential_tests {
    use super::config::{ModelConfigInput, ModelLocation, ModelProtocol};
    use super::credential::CredentialVault;
    use super::{ModelCredentialSource, ModelRuntimeAuthority};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MemoryCredentials(Mutex<HashMap<String, String>>);

    impl CredentialVault for MemoryCredentials {
        fn read(&self, config_id: &str) -> Result<Option<String>, String> {
            Ok(self.0.lock().unwrap().get(config_id).cloned())
        }

        fn set(&self, config_id: &str, api_key: &str) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .insert(config_id.to_owned(), api_key.to_owned());
            Ok(())
        }

        fn delete(&self, config_id: &str) -> Result<(), String> {
            self.0.lock().unwrap().remove(config_id);
            Ok(())
        }
    }

    struct UnavailableCredentials;

    impl CredentialVault for UnavailableCredentials {
        fn read(&self, _config_id: &str) -> Result<Option<String>, String> {
            Err("credential vault unavailable".to_owned())
        }

        fn set(&self, _config_id: &str, _api_key: &str) -> Result<(), String> {
            Err("credential vault unavailable".to_owned())
        }

        fn delete(&self, _config_id: &str) -> Result<(), String> {
            Err("credential vault unavailable".to_owned())
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("aiks-model-authority-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn stores_api_keys_only_in_the_credential_vault_and_removes_them_with_config() {
        let directory = TestDirectory::new();
        let credentials = Arc::new(MemoryCredentials::default());
        let authority = ModelRuntimeAuthority::with_credentials(credentials.clone());
        let state = authority
            .upsert(
                directory.0.clone(),
                ModelConfigInput {
                    config_id: "secure-model".to_owned(),
                    label: "Secure model".to_owned(),
                    location: ModelLocation::Remote,
                    endpoint_url: "https://models.example.com".to_owned(),
                    model: "reasoner".to_owned(),
                    timeout_ms: 30_000,
                    authenticated: true,
                    provider_protocol: ModelProtocol::OpenAi,
                    credential_source: ModelCredentialSource::Keychain,
                    api_key: Some("temporary-test-key".to_owned()),
                },
            )
            .expect("store secure model");
        assert!(state.configs[0].credential_stored);
        assert_eq!(
            state.configs[0].endpoint_url,
            "https://models.example.com/v1/chat/completions"
        );
        let json = fs::read_to_string(directory.0.join("model-runtime-v1.json")).unwrap();
        assert!(!json.contains("temporary-test-key"));
        assert_eq!(
            credentials.read("secure-model").unwrap().as_deref(),
            Some("temporary-test-key")
        );

        authority
            .remove(directory.0.clone(), "secure-model")
            .expect("remove model");
        assert_eq!(credentials.read("secure-model").unwrap(), None);
    }

    #[test]
    fn environment_configs_do_not_require_an_available_system_credential_vault() {
        let directory = TestDirectory::new();
        let authority = ModelRuntimeAuthority::with_credentials(Arc::new(UnavailableCredentials));
        let state = authority
            .upsert(
                directory.0.clone(),
                ModelConfigInput {
                    config_id: "environment-model".to_owned(),
                    label: "Environment model".to_owned(),
                    location: ModelLocation::Local,
                    endpoint_url: "http://127.0.0.1:11434/v1".to_owned(),
                    model: "local".to_owned(),
                    timeout_ms: 30_000,
                    authenticated: true,
                    provider_protocol: ModelProtocol::OpenAi,
                    credential_source: ModelCredentialSource::Environment,
                    api_key: None,
                },
            )
            .expect("store environment config");
        assert_eq!(
            state.configs[0].credential_environment.as_deref(),
            Some("AIKS_MODEL_API_KEY_ENVIRONMENT_MODEL")
        );
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunFileSemanticComparisonRequest {
    proposal_id: String,
    item_id: String,
    desktop_config_id: String,
    agent_config_id: String,
}

pub(crate) fn app_config_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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
pub async fn discover_models(
    request: DiscoverModelsRequest,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<DiscoveredModels, String> {
    let authority = authority.inner().clone();
    tauri::async_runtime::spawn_blocking(move || authority.discover(request))
        .await
        .map_err(|error| format!("Model discovery worker failed: {error}"))?
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

#[tauri::command]
pub(crate) async fn run_file_semantic_comparison(
    request: RunFileSemanticComparisonRequest,
    app: tauri::AppHandle,
    reviewed_sources: tauri::State<'_, crate::discovery::ReviewedSourceRegistry>,
    vaults: tauri::State<'_, crate::vault::VaultAuthorityRegistry>,
    profiles: tauri::State<'_, crate::profiles::ProfileAuthority>,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<FileSemanticComparison, String> {
    let permit = authority.acquire_comparison(&request.proposal_id, &request.item_id)?;
    let now = std::time::Instant::now();
    let source = reviewed_sources
        .resolve_selection_at(
            &request.proposal_id,
            std::slice::from_ref(&request.item_id),
            now,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| "Reviewed source is unavailable".to_owned())?;
    let summary = vaults.current_summary()?;
    let vault = vaults.lease(&summary.authority_id)?;
    let profile = profiles.active_approved_profile_read_only(&vault)?;
    let (desktop_config, agent_config) = authority.load_pair(
        app_config_directory(&app)?,
        &request.desktop_config_id,
        &request.agent_config_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let bytes = file_semantics::read_reviewed_source_bytes(&source)?;
        let comparison = file_semantics::run_file_semantic_comparison(
            &source,
            &bytes,
            &profile,
            &desktop_config,
            &agent_config,
            &OpenAiFileSemanticTransport,
        )?;
        file_semantics::persist_file_semantic_comparison(&vault, &comparison)?;
        Ok(comparison)
    })
    .await
    .map_err(|error| format!("File semantic comparison worker failed: {error}"))?
}

use super::config::ModelConfigSummary;
use super::protocol::{AgentDecision, ProposalSide};
use crate::discovery::{open_trusted_drop_root, CapabilityRoot, ReviewedSource};
use crate::evidence_extraction::{extract_file_evidence, ExtractedFileEvidence};
use crate::identity::ContentIdentity;
use crate::naming::schema::{canonical_policy, NamingFactKind};
use crate::profiles::schema::{ClassificationCategory, DeclarativeProfile, ProfileStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Cursor, Read};
use std::path::Path;
use uuid::Uuid;

const MAX_ENVELOPE_BYTES: usize = 512 * 1024;
const MAX_SUMMARY_CHARS: usize = 2_048;
const MAX_FACT_VALUE_CHARS: usize = 512;
const MAX_NAMING_FACTS: usize = 16;
const MAX_EVIDENCE_IDS: usize = 16;

pub(crate) const FILE_SEMANTIC_POLICY_JSON: &str = r#"{"policyId":"file-classification-naming-v1","version":"1.0.0","task":"fileClassificationAndNaming","requirements":{"classification":"Select at most one categoryId present in the supplied exact taxonomy.","naming":"Return only evidence-supported project, model, regulation, version, and subject facts.","evidence":"Every selected category and naming fact must cite supplied evidenceIds.","uncertainty":"Return no category and explain uncertainty when evidence is insufficient."},"prohibitions":["filesystem mutation","archive mutation","profile mutation","invented categories","invented evidence","approval claims"]}"#;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticPolicySnapshot {
    pub policy_id: String,
    pub version: String,
    pub identity: ContentIdentity,
    pub json: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticProfileSnapshot {
    pub profile_id: String,
    pub version: String,
    pub identity: ContentIdentity,
    pub categories: Vec<ClassificationCategory>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticEnvelope {
    pub schema_version: u32,
    pub task: String,
    pub item_id: String,
    pub original_name: String,
    pub byte_size: u64,
    pub source_identity: ContentIdentity,
    pub profile: FileSemanticProfileSnapshot,
    pub policy: FileSemanticPolicySnapshot,
    pub evidence: ExtractedFileEvidence,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SemanticNamingFact {
    pub kind: NamingFactKind,
    pub value: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticSuggestion {
    pub summary: String,
    pub category_id: Option<String>,
    pub category_evidence_ids: Vec<String>,
    pub naming_facts: Vec<SemanticNamingFact>,
    pub uncertainty_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticAdjudication {
    pub decision: AgentDecision,
    pub reason: String,
    pub evidence_ids: Vec<String>,
    pub selected_side: Option<ProposalSide>,
    pub revised_suggestion: Option<FileSemanticSuggestion>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FileSemanticOutcomeStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticProviderOutcome {
    pub status: FileSemanticOutcomeStatus,
    pub model: Option<String>,
    pub suggestion: Option<FileSemanticSuggestion>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FileSemanticComparisonStatus {
    Completed,
    Review,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileSemanticComparison {
    pub schema_version: u32,
    pub comparison_id: String,
    pub envelope: FileSemanticEnvelope,
    pub envelope_identity: ContentIdentity,
    pub desktop_config_id: String,
    pub agent_config_id: String,
    pub desktop_outcome: FileSemanticProviderOutcome,
    pub agent_outcome: FileSemanticProviderOutcome,
    pub adjudication: Option<FileSemanticAdjudication>,
    pub adjudication_failure: Option<String>,
    pub resolved_suggestion: Option<FileSemanticSuggestion>,
    pub status: FileSemanticComparisonStatus,
}

pub(crate) struct PreparedFileSemanticComparison {
    pub envelope: FileSemanticEnvelope,
    pub identity: ContentIdentity,
    pub json: Vec<u8>,
}

pub(crate) trait FileSemanticTransport: Sync {
    fn propose(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
    ) -> Result<FileSemanticSuggestion, String>;

    fn adjudicate(
        &self,
        config: &ModelConfigSummary,
        envelope_json: &[u8],
        desktop: &FileSemanticSuggestion,
        agent: &FileSemanticSuggestion,
    ) -> Result<FileSemanticAdjudication, String>;
}

pub(crate) fn prepare_file_semantic_comparison(
    source: &ReviewedSource,
    source_bytes: &[u8],
    profile: &DeclarativeProfile,
) -> Result<PreparedFileSemanticComparison, String> {
    profile.validate()?;
    if profile.status != ProfileStatus::Approved {
        return Err("File semantic comparison requires the active approved profile".to_owned());
    }
    if profile.categories.is_empty() {
        return Err("File semantic comparison requires an explicit profile taxonomy".to_owned());
    }
    source.identity.validate()?;
    if source.byte_size != source_bytes.len() as u64 {
        return Err("Reviewed source size changed after discovery".to_owned());
    }
    let evidence = extract_file_evidence(&source.name, source_bytes, &source.identity)?;
    let profile_json = serde_json::to_vec(profile)
        .map_err(|error| format!("Profile snapshot cannot be serialized: {error}"))?;
    let profile_identity = ContentIdentity::from_reader(Cursor::new(profile_json))
        .map_err(|error| format!("Profile snapshot cannot be hashed: {error}"))?;
    let policy_identity =
        ContentIdentity::from_reader(Cursor::new(FILE_SEMANTIC_POLICY_JSON.as_bytes()))
            .map_err(|error| format!("File semantic policy cannot be hashed: {error}"))?;
    let envelope = FileSemanticEnvelope {
        schema_version: 1,
        task: "fileClassificationAndNaming".to_owned(),
        item_id: source.item_id.clone(),
        original_name: source.name.clone(),
        byte_size: source.byte_size,
        source_identity: source.identity.clone(),
        profile: FileSemanticProfileSnapshot {
            profile_id: profile.profile_id.clone(),
            version: profile.version.clone(),
            identity: profile_identity,
            categories: profile.categories.clone(),
        },
        policy: FileSemanticPolicySnapshot {
            policy_id: canonical_policy().policy_id.to_owned(),
            version: canonical_policy().version.to_owned(),
            identity: policy_identity,
            json: FILE_SEMANTIC_POLICY_JSON.to_owned(),
        },
        evidence,
    };
    let json = serde_json::to_vec(&envelope)
        .map_err(|error| format!("File semantic envelope cannot be serialized: {error}"))?;
    if json.len() > MAX_ENVELOPE_BYTES {
        return Err("File semantic envelope exceeds 512 KiB".to_owned());
    }
    let identity = ContentIdentity::from_reader(Cursor::new(&json))
        .map_err(|error| format!("File semantic envelope cannot be hashed: {error}"))?;
    Ok(PreparedFileSemanticComparison {
        envelope,
        identity,
        json,
    })
}

pub(crate) fn read_reviewed_source_bytes(source: &ReviewedSource) -> Result<Vec<u8>, String> {
    source.identity.validate()?;
    if source.byte_size == 0
        || source.byte_size > crate::evidence_extraction::MAX_EVIDENCE_SOURCE_BYTES as u64
    {
        return Err("Reviewed source is empty or exceeds the extraction limit".to_owned());
    }
    let mut file = match open_trusted_drop_root(source.path.clone()) {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Reviewed source is no longer a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { message, .. } => return Err(message),
    };
    let opened = file
        .metadata()
        .map_err(|error| format!("Reviewed source metadata is unavailable: {error}"))?;
    if opened.len() != source.byte_size {
        return Err("Reviewed source size changed after discovery".to_owned());
    }
    let mut bytes = Vec::with_capacity(source.byte_size as usize);
    file.by_ref()
        .take(crate::evidence_extraction::MAX_EVIDENCE_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Reviewed source cannot be read: {error}"))?;
    if bytes.len() != source.byte_size as usize {
        return Err("Reviewed source size changed while reading".to_owned());
    }
    let identity = ContentIdentity::from_reader(Cursor::new(&bytes))
        .map_err(|error| format!("Reviewed source cannot be hashed: {error}"))?;
    if identity != source.identity {
        return Err("Reviewed source identity changed after discovery".to_owned());
    }
    Ok(bytes)
}

pub(crate) fn persist_file_semantic_comparison(
    vault: &crate::vault::VaultLease,
    comparison: &FileSemanticComparison,
) -> Result<(), String> {
    comparison.validate()?;
    crate::vault::records::write_new_json(
        &vault.directory,
        &Path::new(".aiks/file-semantic-comparisons")
            .join(format!("{}.json", comparison.comparison_id)),
        comparison,
    )
}

pub(crate) fn load_file_semantic_comparison(
    vault: &crate::vault::VaultLease,
    comparison_id: &str,
) -> Result<FileSemanticComparison, String> {
    if comparison_id.len() != 32
        || !comparison_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err("File semantic comparison ID is invalid".to_owned());
    }
    let comparison: FileSemanticComparison = crate::vault::records::read_json(
        &vault.directory,
        &Path::new(".aiks/file-semantic-comparisons").join(format!("{comparison_id}.json")),
    )?;
    comparison.validate()?;
    Ok(comparison)
}

pub(crate) fn run_file_semantic_comparison(
    source: &ReviewedSource,
    source_bytes: &[u8],
    profile: &DeclarativeProfile,
    desktop_config: &ModelConfigSummary,
    agent_config: &ModelConfigSummary,
    transport: &dyn FileSemanticTransport,
) -> Result<FileSemanticComparison, String> {
    if desktop_config.config_id == agent_config.config_id {
        return Err("Desktop and Agent model configurations must be distinct".to_owned());
    }
    let prepared = prepare_file_semantic_comparison(source, source_bytes, profile)?;
    let (desktop_result, agent_result) = std::thread::scope(|scope| {
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
    });
    let desktop_outcome = provider_outcome(desktop_config, desktop_result, &prepared.envelope);
    let agent_outcome = provider_outcome(agent_config, agent_result, &prepared.envelope);
    let (adjudication, adjudication_failure, status) = match (
        desktop_outcome.suggestion.as_ref(),
        agent_outcome.suggestion.as_ref(),
    ) {
        (Some(desktop), Some(agent)) => match transport
            .adjudicate(agent_config, &prepared.json, desktop, agent)
            .and_then(|adjudication| {
                adjudication.validate(&prepared.envelope)?;
                Ok(adjudication)
            }) {
            Ok(adjudication) => {
                let status = if matches!(
                    adjudication.decision,
                    AgentDecision::Accept | AgentDecision::Revise
                ) {
                    FileSemanticComparisonStatus::Completed
                } else {
                    FileSemanticComparisonStatus::Review
                };
                (Some(adjudication), None, status)
            }
            Err(error) => (
                None,
                Some(super::bounded_failure(error)),
                FileSemanticComparisonStatus::Review,
            ),
        },
        _ => (None, None, FileSemanticComparisonStatus::Failed),
    };
    let resolved_suggestion = match (
        adjudication.as_ref(),
        desktop_outcome.suggestion.as_ref(),
        agent_outcome.suggestion.as_ref(),
    ) {
        (Some(adjudication), Some(desktop), Some(agent)) => {
            adjudication.resolved(desktop, agent).cloned()
        }
        _ => None,
    };
    Ok(FileSemanticComparison {
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
        resolved_suggestion,
        status,
    })
}

fn provider_outcome(
    config: &ModelConfigSummary,
    result: Result<FileSemanticSuggestion, String>,
    envelope: &FileSemanticEnvelope,
) -> FileSemanticProviderOutcome {
    match result.and_then(|suggestion| {
        suggestion.validate(envelope)?;
        Ok(suggestion)
    }) {
        Ok(suggestion) => FileSemanticProviderOutcome {
            status: FileSemanticOutcomeStatus::Succeeded,
            model: Some(config.model.clone()),
            suggestion: Some(suggestion),
            failure_reason: None,
        },
        Err(error) => FileSemanticProviderOutcome {
            status: FileSemanticOutcomeStatus::Failed,
            model: None,
            suggestion: None,
            failure_reason: Some(super::bounded_failure(error)),
        },
    }
}

impl FileSemanticSuggestion {
    pub(crate) fn validate(&self, envelope: &FileSemanticEnvelope) -> Result<(), String> {
        validate_text(&self.summary, MAX_SUMMARY_CHARS, "Suggestion summary")?;
        let available_evidence = evidence_ids(envelope);
        match (&self.category_id, &self.uncertainty_reason) {
            (Some(category_id), None) => {
                if !envelope
                    .profile
                    .categories
                    .iter()
                    .any(|category| &category.category_id == category_id)
                {
                    return Err("Suggestion category is absent from the exact profile".to_owned());
                }
                validate_evidence_ids(&self.category_evidence_ids, &available_evidence)?;
            }
            (None, Some(reason)) => {
                if !self.category_evidence_ids.is_empty() {
                    return Err("Uncertain suggestion cannot claim category evidence".to_owned());
                }
                validate_text(reason, MAX_SUMMARY_CHARS, "Uncertainty reason")?;
            }
            _ => {
                return Err(
                    "Suggestion must select one supported category or explain uncertainty"
                        .to_owned(),
                )
            }
        }
        if self.naming_facts.len() > MAX_NAMING_FACTS {
            return Err("Suggestion exceeds the naming fact limit".to_owned());
        }
        let mut facts = HashSet::with_capacity(self.naming_facts.len());
        for fact in &self.naming_facts {
            validate_text(&fact.value, MAX_FACT_VALUE_CHARS, "Naming fact value")?;
            validate_evidence_ids(&fact.evidence_ids, &available_evidence)?;
            if !facts.insert((fact.kind, fact.value.as_str())) {
                return Err("Suggestion contains duplicate naming facts".to_owned());
            }
        }
        Ok(())
    }
}

impl FileSemanticAdjudication {
    pub(crate) fn validate(&self, envelope: &FileSemanticEnvelope) -> Result<(), String> {
        validate_text(&self.reason, MAX_SUMMARY_CHARS, "Adjudication reason")?;
        validate_evidence_ids(&self.evidence_ids, &evidence_ids(envelope))?;
        match self.decision {
            AgentDecision::Accept
                if self.selected_side.is_some() && self.revised_suggestion.is_none() => {}
            AgentDecision::Revise
                if self.selected_side.is_none() && self.revised_suggestion.is_some() =>
            {
                self.revised_suggestion
                    .as_ref()
                    .expect("revised suggestion exists")
                    .validate(envelope)?;
            }
            AgentDecision::Reject | AgentDecision::Review
                if self.selected_side.is_none() && self.revised_suggestion.is_none() => {}
            _ => return Err("Agent adjudication shape does not match its decision".to_owned()),
        }
        Ok(())
    }

    pub(crate) fn resolved<'a>(
        &'a self,
        desktop: &'a FileSemanticSuggestion,
        agent: &'a FileSemanticSuggestion,
    ) -> Option<&'a FileSemanticSuggestion> {
        match (self.decision, self.selected_side) {
            (AgentDecision::Accept, Some(ProposalSide::Desktop)) => Some(desktop),
            (AgentDecision::Accept, Some(ProposalSide::Agent)) => Some(agent),
            (AgentDecision::Revise, None) => self.revised_suggestion.as_ref(),
            _ => None,
        }
    }
}

impl FileSemanticComparison {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.comparison_id.len() != 32
            || !self
                .comparison_id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || self.desktop_config_id == self.agent_config_id
        {
            return Err("File semantic comparison identity is invalid".to_owned());
        }
        let envelope_json = serde_json::to_vec(&self.envelope)
            .map_err(|error| format!("File semantic envelope cannot be serialized: {error}"))?;
        let envelope_identity = ContentIdentity::from_reader(Cursor::new(envelope_json))
            .map_err(|error| format!("File semantic envelope cannot be hashed: {error}"))?;
        if envelope_identity != self.envelope_identity
            || self.envelope.source_identity != self.envelope.evidence.source_identity
        {
            return Err("File semantic comparison envelope identity is invalid".to_owned());
        }
        validate_outcome(&self.desktop_outcome, &self.envelope)?;
        validate_outcome(&self.agent_outcome, &self.envelope)?;
        let expected_resolved = match (
            self.adjudication.as_ref(),
            self.desktop_outcome.suggestion.as_ref(),
            self.agent_outcome.suggestion.as_ref(),
        ) {
            (Some(adjudication), Some(desktop), Some(agent)) => {
                adjudication.resolved(desktop, agent)
            }
            _ => None,
        };
        if expected_resolved != self.resolved_suggestion.as_ref() {
            return Err("Resolved file semantic suggestion is inconsistent".to_owned());
        }
        match (&self.adjudication, &self.adjudication_failure, self.status) {
            (Some(adjudication), None, FileSemanticComparisonStatus::Completed)
            | (Some(adjudication), None, FileSemanticComparisonStatus::Review) => {
                adjudication.validate(&self.envelope)?;
            }
            (None, Some(reason), FileSemanticComparisonStatus::Review) => {
                validate_text(reason, MAX_SUMMARY_CHARS, "Adjudication failure")?;
            }
            (None, None, FileSemanticComparisonStatus::Failed) => {}
            _ => return Err("File semantic comparison outcome is inconsistent".to_owned()),
        }
        Ok(())
    }
}

fn validate_outcome(
    outcome: &FileSemanticProviderOutcome,
    envelope: &FileSemanticEnvelope,
) -> Result<(), String> {
    match outcome.status {
        FileSemanticOutcomeStatus::Succeeded => {
            if outcome.failure_reason.is_some() {
                return Err("Successful semantic outcome cannot include a failure".to_owned());
            }
            validate_text(
                outcome.model.as_deref().unwrap_or_default(),
                MAX_FACT_VALUE_CHARS,
                "Provider model",
            )?;
            outcome
                .suggestion
                .as_ref()
                .ok_or_else(|| "Successful semantic outcome requires a suggestion".to_owned())?
                .validate(envelope)
        }
        FileSemanticOutcomeStatus::Failed => {
            if outcome.model.is_some() || outcome.suggestion.is_some() {
                return Err("Failed semantic outcome cannot include a suggestion".to_owned());
            }
            validate_text(
                outcome.failure_reason.as_deref().unwrap_or_default(),
                MAX_SUMMARY_CHARS,
                "Provider failure",
            )
        }
    }
}

fn evidence_ids(envelope: &FileSemanticEnvelope) -> HashSet<&str> {
    envelope
        .evidence
        .excerpts
        .iter()
        .map(|excerpt| excerpt.evidence_id.as_str())
        .collect()
}

fn validate_evidence_ids(values: &[String], available: &HashSet<&str>) -> Result<(), String> {
    if values.is_empty() || values.len() > MAX_EVIDENCE_IDS {
        return Err("Semantic output must cite 1 to 16 evidence IDs".to_owned());
    }
    let mut unique = HashSet::with_capacity(values.len());
    if values
        .iter()
        .any(|value| !available.contains(value.as_str()) || !unique.insert(value.as_str()))
    {
        return Err("Semantic output contains an invalid evidence ID".to_owned());
    }
    Ok(())
}

fn validate_text(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.chars().count() > maximum
        || value.chars().any(char::is_control)
    {
        return Err(format!("{label} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        load_file_semantic_comparison, persist_file_semantic_comparison,
        prepare_file_semantic_comparison, run_file_semantic_comparison, FileSemanticAdjudication,
        FileSemanticComparisonStatus, FileSemanticSuggestion, FileSemanticTransport,
        SemanticNamingFact,
    };
    use crate::discovery::ReviewedSource;
    use crate::identity::ContentIdentity;
    use crate::model_runtime::config::{ModelConfigSummary, ModelLocation};
    use crate::model_runtime::protocol::{AgentDecision, ProposalSide};
    use crate::naming::schema::NamingFactKind;
    use crate::profiles::schema::{
        ClassificationCategory, DeclarativeProfile, IndependentNodeTrigger, ProfileGovernance,
        ProfileOwnership, ProfileProvenance, ProfileStatus, ReviewDisposition,
    };
    use cap_std::{ambient_authority, fs::Dir};
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{Instant, SystemTime};

    struct FakeTransport {
        envelopes: Mutex<Vec<Vec<u8>>>,
        desktop: FileSemanticSuggestion,
        agent: FileSemanticSuggestion,
        adjudication: FileSemanticAdjudication,
    }

    impl FileSemanticTransport for FakeTransport {
        fn propose(
            &self,
            config: &ModelConfigSummary,
            envelope_json: &[u8],
        ) -> Result<FileSemanticSuggestion, String> {
            self.envelopes
                .lock()
                .expect("captured file envelopes")
                .push(envelope_json.to_vec());
            if config.config_id == "desktop" {
                Ok(self.desktop.clone())
            } else {
                Ok(self.agent.clone())
            }
        }

        fn adjudicate(
            &self,
            _config: &ModelConfigSummary,
            envelope_json: &[u8],
            _desktop: &FileSemanticSuggestion,
            _agent: &FileSemanticSuggestion,
        ) -> Result<FileSemanticAdjudication, String> {
            self.envelopes
                .lock()
                .expect("captured adjudication envelope")
                .push(envelope_json.to_vec());
            Ok(self.adjudication.clone())
        }
    }

    fn identity(bytes: &[u8]) -> ContentIdentity {
        ContentIdentity::from_reader(Cursor::new(bytes)).expect("hash file semantic fixture")
    }

    fn source(bytes: &[u8]) -> ReviewedSource {
        ReviewedSource {
            item_id: "item-1".to_owned(),
            path: PathBuf::from("/review/report.md"),
            name: "000123.md".to_owned(),
            byte_size: bytes.len() as u64,
            identity: identity(bytes),
        }
    }

    fn source_at(path: &Path, bytes: &[u8]) -> ReviewedSource {
        ReviewedSource {
            item_id: "item-1".to_owned(),
            path: path.to_owned(),
            name: "000123.md".to_owned(),
            byte_size: bytes.len() as u64,
            identity: identity(bytes),
        }
    }

    fn profile(status: ProfileStatus) -> DeclarativeProfile {
        DeclarativeProfile {
            schema_version: 2,
            profile_id: "fixture-taxonomy".to_owned(),
            version: "1.0.0".to_owned(),
            title: "Fixture taxonomy".to_owned(),
            status,
            provenance: ProfileProvenance {
                source_title: "Owned fixture".to_owned(),
                ownership: ProfileOwnership::Owned,
                evidence: vec!["authorization:test".to_owned()],
            },
            categories: vec![
                ClassificationCategory {
                    category_id: "research".to_owned(),
                    label: "Research".to_owned(),
                    depth: 1,
                    parent_id: None,
                    path: vec!["Research".to_owned()],
                    aliases: Vec::new(),
                },
                ClassificationCategory {
                    category_id: "research.reports".to_owned(),
                    label: "Reports".to_owned(),
                    depth: 2,
                    parent_id: Some("research".to_owned()),
                    path: vec!["Research".to_owned(), "Reports".to_owned()],
                    aliases: Vec::new(),
                },
            ],
            governance: Some(ProfileGovernance {
                maximum_depth: 4,
                unique_primary_archive_category: true,
                semantic_evidence_required: true,
                metadata_only_dimensions: vec!["project".to_owned()],
                insufficient_evidence_disposition: ReviewDisposition::ImportantIndexed,
                conflicting_evidence_disposition: ReviewDisposition::ClassificationReview,
                archive_first: true,
                cross_domain_knowledge_links: true,
                independent_node_triggers: vec![IndependentNodeTrigger::HighValue],
                generated_indexes_link_only: true,
            }),
            rules: Vec::new(),
        }
    }

    fn config(config_id: &str) -> ModelConfigSummary {
        ModelConfigSummary {
            config_id: config_id.to_owned(),
            label: config_id.to_owned(),
            location: ModelLocation::Local,
            endpoint_url: "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
            model: format!("{config_id}-model"),
            timeout_ms: 30_000,
            authenticated: false,
            credential_environment: None,
        }
    }

    fn suggestion(evidence_id: &str, subject: &str) -> FileSemanticSuggestion {
        FileSemanticSuggestion {
            summary: "Evidence supports one report category".to_owned(),
            category_id: Some("research.reports".to_owned()),
            category_evidence_ids: vec![evidence_id.to_owned()],
            naming_facts: vec![SemanticNamingFact {
                kind: NamingFactKind::Subject,
                value: subject.to_owned(),
                evidence_ids: vec![evidence_id.to_owned()],
            }],
            uncertainty_reason: None,
        }
    }

    #[test]
    fn builds_one_exact_identity_bound_envelope_for_both_models() {
        let bytes = b"Project Atlas reset reliability report";
        let prepared = prepare_file_semantic_comparison(
            &source(bytes),
            bytes,
            &profile(ProfileStatus::Approved),
        )
        .expect("prepare file semantic comparison");
        let evidence_id = prepared.envelope.evidence.excerpts[0].evidence_id.clone();
        let transport = FakeTransport {
            envelopes: Mutex::new(Vec::new()),
            desktop: suggestion(&evidence_id, "Reset reliability"),
            agent: suggestion(&evidence_id, "MCU reset reliability"),
            adjudication: FileSemanticAdjudication {
                decision: AgentDecision::Accept,
                reason: "Agent proposal is more specific and evidence supported".to_owned(),
                evidence_ids: vec![evidence_id],
                selected_side: Some(ProposalSide::Agent),
                revised_suggestion: None,
            },
        };

        let result = run_file_semantic_comparison(
            &source(bytes),
            bytes,
            &profile(ProfileStatus::Approved),
            &config("desktop"),
            &config("agent"),
            &transport,
        )
        .expect("run file semantic comparison");

        assert_eq!(result.status, FileSemanticComparisonStatus::Completed);
        assert_eq!(result.envelope.source_identity, identity(bytes));
        let captured = transport.envelopes.lock().expect("captured envelopes");
        assert_eq!(captured.len(), 3);
        assert_eq!(captured[0], captured[1]);
        assert_eq!(captured[1], captured[2]);
        let resolved = result
            .adjudication
            .as_ref()
            .expect("Agent adjudication")
            .resolved(
                result
                    .desktop_outcome
                    .suggestion
                    .as_ref()
                    .expect("desktop suggestion"),
                result
                    .agent_outcome
                    .suggestion
                    .as_ref()
                    .expect("Agent suggestion"),
            )
            .expect("resolved suggestion");
        assert_eq!(resolved.naming_facts[0].value, "MCU reset reliability");
    }

    #[test]
    fn rejects_draft_profiles_invented_categories_and_uncited_output() {
        let bytes = b"Project Atlas reset reliability report";
        assert!(prepare_file_semantic_comparison(
            &source(bytes),
            bytes,
            &profile(ProfileStatus::Draft),
        )
        .is_err());
        let prepared = prepare_file_semantic_comparison(
            &source(bytes),
            bytes,
            &profile(ProfileStatus::Approved),
        )
        .expect("prepare approved comparison");
        let evidence_id = prepared.envelope.evidence.excerpts[0].evidence_id.clone();
        let mut invented = suggestion(&evidence_id, "Reset reliability");
        invented.category_id = Some("invented.category".to_owned());
        assert!(invented.validate(&prepared.envelope).is_err());
        let mut uncited = suggestion(&evidence_id, "Reset reliability");
        uncited.naming_facts[0].evidence_ids = vec!["invented-evidence".to_owned()];
        assert!(uncited.validate(&prepared.envelope).is_err());
    }

    #[test]
    fn agent_review_never_resolves_to_an_actionable_suggestion() {
        let bytes = b"Ambiguous document";
        let prepared = prepare_file_semantic_comparison(
            &source(bytes),
            bytes,
            &profile(ProfileStatus::Approved),
        )
        .expect("prepare comparison");
        let evidence_id = prepared.envelope.evidence.excerpts[0].evidence_id.clone();
        let desktop = suggestion(&evidence_id, "Document");
        let agent = suggestion(&evidence_id, "Ambiguous document");
        let adjudication = FileSemanticAdjudication {
            decision: AgentDecision::Review,
            reason: "The evidence is insufficient to choose between the proposals".to_owned(),
            evidence_ids: vec![evidence_id],
            selected_side: None,
            revised_suggestion: None,
        };

        adjudication
            .validate(&prepared.envelope)
            .expect("validate review decision");
        assert!(adjudication.resolved(&desktop, &agent).is_none());
    }

    #[test]
    fn persists_one_immutable_record_without_mutating_the_source() {
        let bytes = b"Project Atlas reset reliability report";
        let root = std::env::temp_dir().join(format!(
            "aiks-file-semantic-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let vault_path = root.join("vault");
        let source_path = root.join("000123.md");
        fs::create_dir_all(vault_path.join(".aiks/file-semantic-comparisons"))
            .expect("create semantic comparison directory");
        fs::write(&source_path, bytes).expect("write reviewed source");
        let reviewed = source_at(&source_path, bytes);
        let prepared =
            prepare_file_semantic_comparison(&reviewed, bytes, &profile(ProfileStatus::Approved))
                .expect("prepare file semantic comparison");
        let evidence_id = prepared.envelope.evidence.excerpts[0].evidence_id.clone();
        let transport = FakeTransport {
            envelopes: Mutex::new(Vec::new()),
            desktop: suggestion(&evidence_id, "Reset reliability"),
            agent: suggestion(&evidence_id, "MCU reset reliability"),
            adjudication: FileSemanticAdjudication {
                decision: AgentDecision::Accept,
                reason: "Agent proposal is more specific and evidence supported".to_owned(),
                evidence_ids: vec![evidence_id],
                selected_side: Some(ProposalSide::Agent),
                revised_suggestion: None,
            },
        };
        let comparison = run_file_semantic_comparison(
            &reviewed,
            bytes,
            &profile(ProfileStatus::Approved),
            &config("desktop"),
            &config("agent"),
            &transport,
        )
        .expect("run comparison");
        let vault = crate::vault::VaultLease {
            summary: crate::vault::VaultSummary {
                authority_id: "test-vault".to_owned(),
                display_path: vault_path.to_string_lossy().into_owned(),
                status: crate::vault::VaultStatus::Authoritative,
            },
            directory: Dir::open_ambient_dir(&vault_path, ambient_authority())
                .expect("open test Vault"),
        };

        persist_file_semantic_comparison(&vault, &comparison).expect("persist comparison");
        assert_eq!(
            load_file_semantic_comparison(&vault, &comparison.comparison_id)
                .expect("reload comparison"),
            comparison
        );
        assert_eq!(
            fs::read(&source_path).expect("read unchanged source"),
            bytes
        );
        assert!(persist_file_semantic_comparison(&vault, &comparison).is_err());

        let batch = crate::profiles::ClassificationBatchRegistry::default()
            .create_semantic_at(
                "proposal-1",
                &profile(ProfileStatus::Approved),
                vec![reviewed],
                vec![comparison.clone()],
                Instant::now(),
                SystemTime::now(),
            )
            .expect("create semantic classification batch");
        let proposal = &batch.items[0].proposal;
        assert!(proposal.rule_ids.is_empty());
        assert_eq!(
            proposal.semantic_decision_id.as_deref(),
            Some(comparison.comparison_id.as_str())
        );
        assert_eq!(
            proposal.destination.as_deref(),
            Some(["Research".to_owned(), "Reports".to_owned()].as_slice())
        );
        assert!(proposal.committable);
        let mut altered_profile = profile(ProfileStatus::Approved);
        altered_profile.title = "Altered fixture taxonomy".to_owned();
        assert!(crate::profiles::ClassificationBatchRegistry::default()
            .create_semantic_at(
                "proposal-1",
                &altered_profile,
                vec![source_at(&source_path, bytes)],
                vec![comparison.clone()],
                Instant::now(),
                SystemTime::now(),
            )
            .is_err());

        let mut inconsistent = comparison.clone();
        inconsistent.comparison_id = "b".repeat(32);
        inconsistent.resolved_suggestion = inconsistent.desktop_outcome.suggestion.clone();
        assert!(persist_file_semantic_comparison(&vault, &inconsistent).is_err());
        assert!(!vault_path
            .join(".aiks/file-semantic-comparisons")
            .join(format!("{}.json", inconsistent.comparison_id))
            .exists());

        drop(vault);
        fs::remove_dir_all(root).expect("remove file semantic test directory");
    }
}

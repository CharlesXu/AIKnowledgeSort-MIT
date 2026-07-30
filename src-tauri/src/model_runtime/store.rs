#[cfg(test)]
mod tests {
    use super::{
        build_comparison_envelope, inspect_comparison_records, persist_comparison_record,
        EvidenceRange,
    };
    use crate::archive::{
        commit_plan_with_faults, ArchivePlan, ArchivePlanItem, TransactionFaults,
    };
    use crate::identity::ContentIdentity;
    use crate::knowledge::save_document;
    use crate::model_runtime::config::{
        ModelConfigSummary, ModelCredentialSource, ModelLocation, ModelProtocol,
    };
    use crate::model_runtime::protocol::{
        AgentAdjudication, AgentDecision, ComparisonRecord, ComparisonStatus, ModelProposal,
        ProviderOutcome, RelationSuggestion,
    };
    use crate::model_runtime::{run_comparison_with_transport, ModelTransport};
    use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
    use crate::vault::VaultAuthorityRegistry;
    use std::fs;
    use std::io::Cursor;
    use std::ops::Deref;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SOURCE_BYTES: &[u8] = b"comparison source bytes\n";

    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        archived: PathBuf,
        lease: FixtureLease,
        operation_id: String,
    }

    struct FixtureLease {
        lease: Option<crate::vault::VaultLease>,
        root: PathBuf,
    }

    impl Deref for FixtureLease {
        type Target = crate::vault::VaultLease;

        fn deref(&self) -> &Self::Target {
            self.lease.as_ref().expect("fixture lease is available")
        }
    }

    impl Drop for FixtureLease {
        fn drop(&mut self) {
            drop(self.lease.take());
            fs::remove_dir_all(&self.root).expect("remove comparison fixture");
        }
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aiks-model-comparison-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&root).expect("create comparison fixture");
            let root = root.canonicalize().expect("canonical comparison fixture");
            let source = root.join("source.txt");
            fs::write(&source, SOURCE_BYTES).expect("write source fixture");
            let vault_path = root.join("vault");
            fs::create_dir(&vault_path).expect("create fixture Vault");
            let vaults = VaultAuthorityRegistry::default();
            let summary = vaults.authorize_path(&vault_path).expect("authorize Vault");
            let lease = vaults.lease(&summary.authority_id).expect("lease Vault");
            let identity = ContentIdentity::from_reader(Cursor::new(SOURCE_BYTES))
                .expect("hash source fixture");
            let destination_path = format!("Originals/{}/Comparison-source.txt", identity.digest);
            let plan = ArchivePlan {
                plan_id: "comparison-plan".to_owned(),
                plan_version: 2,
                proposal_id: "comparison-proposal".to_owned(),
                naming_batch_id: "comparison-naming".to_owned(),
                classification_batch_id: None,
                authority_id: summary.authority_id,
                vault_path: vault_path.to_string_lossy().into_owned(),
                expires_at_unix_ms: u64::MAX,
                confirmation_nonce: "comparison-confirmation".to_owned(),
                source_preserved: true,
                items: vec![ArchivePlanItem {
                    item_id: "comparison-item".to_owned(),
                    source_path: source.to_string_lossy().into_owned(),
                    destination_path: destination_path.clone(),
                    original_name: "source.txt".to_owned(),
                    canonical_name: "Comparison-source.txt".to_owned(),
                    classification: None,
                    naming: NamingDecisionEvidence {
                        naming_proposal_id: "comparison-name".to_owned(),
                        original_name: "source.txt".to_owned(),
                        canonical_name: "Comparison-source.txt".to_owned(),
                        policy_id: "canonical-v1".to_owned(),
                        policy_version: "1.0.0".to_owned(),
                        applied_rule: "ordered-cited-facts-v1".to_owned(),
                        facts: vec![NamingFact {
                            kind: NamingFactKind::Subject,
                            value: "Comparison source".to_owned(),
                            evidence_location: "page:1".to_owned(),
                        }],
                    },
                    byte_size: SOURCE_BYTES.len() as u64,
                    identity,
                }],
            };
            let committed = commit_plan_with_faults(plan, &lease, TransactionFaults::default());
            let operation_id = committed.items[0].operation_id.clone();
            save_document(
                &lease,
                &operation_id,
                0,
                "# First\nEvidence alpha.\nEvidence beta.\nTail.\n",
            )
            .expect("save first revision");
            save_document(&lease, &operation_id, 1, "# Second\nDifferent evidence.\n")
                .expect("save second revision");
            Self {
                root: root.clone(),
                source,
                archived: vault_path.join(destination_path),
                lease: FixtureLease {
                    lease: Some(lease),
                    root,
                },
                operation_id,
            }
        }
    }

    fn range(start_line: u32, end_line: u32) -> EvidenceRange {
        EvidenceRange {
            start_line,
            end_line,
        }
    }

    #[test]
    fn builds_deterministic_envelope_from_one_exact_verified_revision() {
        let fixture = Fixture::new();
        let first =
            build_comparison_envelope(&fixture.lease, &fixture.operation_id, 1, &[range(2, 3)])
                .expect("build comparison envelope");
        let repeated =
            build_comparison_envelope(&fixture.lease, &fixture.operation_id, 1, &[range(2, 3)])
                .expect("repeat comparison envelope");

        assert_eq!(first.json, repeated.json);
        assert_eq!(first.identity, repeated.identity);
        assert_eq!(first.envelope.knowledge_revision, 1);
        assert_eq!(first.envelope.evidence[0].evidence_id, "line-2-3");
        assert_eq!(
            first.envelope.evidence[0].text,
            "Evidence alpha.\nEvidence beta.\n"
        );
        assert_eq!(
            first.envelope.rule_snapshot.policy_id,
            "knowledge-relations-v1"
        );
        first
            .envelope
            .rule_snapshot
            .identity
            .validate()
            .expect("validate rule identity");
        assert_eq!(
            fs::read(&fixture.source).expect("read source"),
            SOURCE_BYTES
        );
    }

    #[test]
    fn rejects_invalid_ranges_revisions_and_oversized_envelopes_without_a_record() {
        let fixture = Fixture::new();
        for (revision, ranges) in [
            (0, vec![range(1, 1)]),
            (3, vec![range(1, 1)]),
            (1, vec![]),
            (1, vec![range(0, 1)]),
            (1, vec![range(3, 2)]),
            (1, vec![range(2, 9)]),
            (1, vec![range(2, 2), range(2, 2)]),
            (1, (1..=17).map(|line| range(line, line)).collect()),
        ] {
            assert!(build_comparison_envelope(
                &fixture.lease,
                &fixture.operation_id,
                revision,
                &ranges,
            )
            .is_err());
        }

        let oversized = format!("# Large\n{}\n", "x".repeat(130 * 1024));
        save_document(&fixture.lease, &fixture.operation_id, 2, &oversized)
            .expect("save oversized envelope source revision");
        assert!(build_comparison_envelope(
            &fixture.lease,
            &fixture.operation_id,
            3,
            &[range(2, 2)],
        )
        .is_err());
        assert_eq!(
            fs::read_dir(fixture.root.join("vault/.aiks/comparisons"))
                .expect("read comparison directory")
                .count(),
            0
        );
    }

    #[test]
    fn rejects_tampered_markdown_and_archived_original() {
        let fixture = Fixture::new();
        let markdown = fixture
            .root
            .join("vault/Knowledge")
            .join(&fixture.operation_id)
            .join("00000001.md");
        fs::write(markdown, "# Tampered\n").expect("tamper Markdown");
        assert!(build_comparison_envelope(
            &fixture.lease,
            &fixture.operation_id,
            1,
            &[range(1, 1)],
        )
        .is_err());

        let fixture = Fixture::new();
        fs::write(&fixture.archived, b"tampered original\n").expect("tamper original");
        assert!(build_comparison_envelope(
            &fixture.lease,
            &fixture.operation_id,
            1,
            &[range(1, 1)],
        )
        .is_err());
    }

    #[test]
    fn persists_one_immutable_comparison_record() {
        let fixture = Fixture::new();
        let prepared =
            build_comparison_envelope(&fixture.lease, &fixture.operation_id, 1, &[range(2, 3)])
                .expect("build envelope");
        let record = ComparisonRecord {
            schema_version: 1,
            comparison_id: uuid::Uuid::new_v4().simple().to_string(),
            envelope: prepared.envelope,
            envelope_identity: prepared.identity,
            desktop_config_id: "desktop-model".to_owned(),
            agent_config_id: "agent-model".to_owned(),
            desktop_outcome: ProviderOutcome::failed("not run"),
            agent_outcome: ProviderOutcome::failed("not run"),
            adjudication: None,
            adjudication_failure: None,
            status: ComparisonStatus::Failed,
            actor: "desktop-orchestrator".to_owned(),
            recorded_at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_millis() as u64,
        };

        persist_comparison_record(&fixture.lease, &record).expect("persist record");
        assert!(persist_comparison_record(&fixture.lease, &record).is_err());
        assert_eq!(
            inspect_comparison_records(&fixture.lease).expect("inspect records"),
            vec![record.clone()]
        );
        assert!(fixture
            .root
            .join("vault/.aiks/comparisons")
            .join(&record.comparison_id)
            .join("00000001.json")
            .is_file());
    }

    type CapturedAdjudication = (String, Vec<u8>, ModelProposal, ModelProposal);

    struct CaptureTransport {
        proposals_started: Barrier,
        proposal_envelopes: Mutex<Vec<(String, Vec<u8>)>>,
        adjudications: Mutex<Vec<CapturedAdjudication>>,
    }

    impl CaptureTransport {
        fn new() -> Self {
            Self {
                proposals_started: Barrier::new(2),
                proposal_envelopes: Mutex::new(Vec::new()),
                adjudications: Mutex::new(Vec::new()),
            }
        }
    }

    impl ModelTransport for CaptureTransport {
        fn propose(
            &self,
            config: &ModelConfigSummary,
            envelope_json: &[u8],
        ) -> Result<ModelProposal, String> {
            self.proposal_envelopes
                .lock()
                .expect("lock proposal capture")
                .push((config.config_id.clone(), envelope_json.to_vec()));
            self.proposals_started.wait();
            Ok(ModelProposal {
                summary: format!("{} proposal", config.config_id),
                relations: vec![RelationSuggestion {
                    source: "MCU".to_owned(),
                    relation_type: if config.config_id == "desktop-model" {
                        "dependsOn".to_owned()
                    } else {
                        "relatedTo".to_owned()
                    },
                    target: "Reset controller".to_owned(),
                    evidence_ids: vec!["line-2-3".to_owned()],
                }],
            })
        }

        fn adjudicate(
            &self,
            config: &ModelConfigSummary,
            envelope_json: &[u8],
            desktop: &ModelProposal,
            agent: &ModelProposal,
        ) -> Result<AgentAdjudication, String> {
            self.adjudications
                .lock()
                .expect("lock adjudication capture")
                .push((
                    config.config_id.clone(),
                    envelope_json.to_vec(),
                    desktop.clone(),
                    agent.clone(),
                ));
            Ok(AgentAdjudication {
                decision: AgentDecision::Review,
                reason: "The relation types materially conflict".to_owned(),
                evidence_ids: vec!["line-2-3".to_owned()],
                selected_side: None,
                revised_relations: vec![],
            })
        }
    }

    fn model_config(config_id: &str) -> ModelConfigSummary {
        ModelConfigSummary {
            config_id: config_id.to_owned(),
            label: config_id.to_owned(),
            location: ModelLocation::Local,
            endpoint_url: "http://127.0.0.1:11434/v1/chat/completions".to_owned(),
            model: format!("{config_id}-v1"),
            timeout_ms: 5_000,
            authenticated: false,
            provider_protocol: ModelProtocol::OpenAi,
            credential_source: ModelCredentialSource::Environment,
            credential_environment: None,
            credential_stored: false,
            credential_value: None,
        }
    }

    #[test]
    fn runs_independent_identical_proposals_then_agent_only_adjudication() {
        let fixture = Fixture::new();
        let transport = Arc::new(CaptureTransport::new());
        let source_before = fs::read(&fixture.source).expect("snapshot source");
        let archive_before = fs::read(&fixture.archived).expect("snapshot archive");
        let graph_before = fs::read_dir(fixture.root.join("vault/.aiks/graph/relations"))
            .expect("snapshot graph")
            .count();

        let record = run_comparison_with_transport(
            &fixture.lease,
            &fixture.operation_id,
            1,
            &[range(2, 3)],
            &model_config("desktop-model"),
            &model_config("agent-model"),
            transport.as_ref(),
        )
        .expect("run comparison");

        assert_eq!(record.status, ComparisonStatus::Review);
        let proposals = transport
            .proposal_envelopes
            .lock()
            .expect("read proposal capture");
        assert_eq!(proposals.len(), 2);
        assert_eq!(proposals[0].1, proposals[1].1);
        let adjudications = transport
            .adjudications
            .lock()
            .expect("read adjudication capture");
        assert_eq!(adjudications.len(), 1);
        assert_eq!(adjudications[0].0, "agent-model");
        assert_eq!(adjudications[0].1, proposals[0].1);
        assert_eq!(
            fs::read(&fixture.source).expect("reread source"),
            source_before
        );
        assert_eq!(
            fs::read(&fixture.archived).expect("reread archive"),
            archive_before
        );
        assert_eq!(
            fs::read_dir(fixture.root.join("vault/.aiks/graph/relations"))
                .expect("reread graph")
                .count(),
            graph_before
        );
    }

    #[test]
    fn rejects_same_config_before_model_execution() {
        let fixture = Fixture::new();
        let transport = CaptureTransport::new();
        let config = model_config("same-model");
        assert!(run_comparison_with_transport(
            &fixture.lease,
            &fixture.operation_id,
            1,
            &[range(2, 3)],
            &config,
            &config,
            &transport,
        )
        .is_err());
        assert!(transport
            .proposal_envelopes
            .lock()
            .expect("read proposal capture")
            .is_empty());
    }

    struct FailureTransport {
        adjudication_calls: AtomicUsize,
    }

    impl ModelTransport for FailureTransport {
        fn propose(
            &self,
            config: &ModelConfigSummary,
            _envelope_json: &[u8],
        ) -> Result<ModelProposal, String> {
            if config.config_id == "desktop-model" {
                return Err("deadline exceeded".to_owned());
            }
            Ok(ModelProposal {
                summary: "Agent proposal".to_owned(),
                relations: vec![RelationSuggestion {
                    source: "MCU".to_owned(),
                    relation_type: "dependsOn".to_owned(),
                    target: "Reset controller".to_owned(),
                    evidence_ids: vec!["line-2-3".to_owned()],
                }],
            })
        }

        fn adjudicate(
            &self,
            _config: &ModelConfigSummary,
            _envelope_json: &[u8],
            _desktop: &ModelProposal,
            _agent: &ModelProposal,
        ) -> Result<AgentAdjudication, String> {
            self.adjudication_calls.fetch_add(1, Ordering::Relaxed);
            Err("adjudication must not run".to_owned())
        }
    }

    #[test]
    fn persists_provider_failure_without_agent_adjudication_or_other_mutation() {
        let fixture = Fixture::new();
        let transport = FailureTransport {
            adjudication_calls: AtomicUsize::new(0),
        };
        let archive_before = fs::read(&fixture.archived).expect("snapshot archive");
        let record = run_comparison_with_transport(
            &fixture.lease,
            &fixture.operation_id,
            1,
            &[range(2, 3)],
            &model_config("desktop-model"),
            &model_config("agent-model"),
            &transport,
        )
        .expect("record failed comparison");

        assert_eq!(record.status, ComparisonStatus::Failed);
        assert_eq!(
            record.desktop_outcome.failure_reason.as_deref(),
            Some("deadline exceeded")
        );
        assert!(record.adjudication.is_none());
        assert_eq!(transport.adjudication_calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            fs::read(&fixture.archived).expect("reread archive"),
            archive_before
        );
        assert_eq!(
            inspect_comparison_records(&fixture.lease)
                .expect("inspect failed record")
                .len(),
            1
        );
    }
}
use super::protocol::{
    ComparisonEnvelope, ComparisonRecord, ComparisonStatus, ComparisonTask, EvidenceExcerpt,
    ProviderOutcomeStatus, RuleSnapshot, KNOWLEDGE_RELATION_RULE_JSON,
};
use crate::identity::ContentIdentity;
use crate::knowledge::open_committed_revision;
use crate::vault::records::{read_json, write_new_json};
use crate::vault::VaultLease;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, Cursor};
use std::path::Path;

const COMPARISON_SCHEMA_VERSION: u32 = 1;
const MAX_EVIDENCE_RANGES: usize = 16;
const MAX_ENVELOPE_BYTES: usize = 128 * 1024;
const MAX_COMPARISON_RECORDS: usize = 10_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRange {
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Clone, Debug)]
pub struct PreparedComparison {
    pub envelope: ComparisonEnvelope,
    pub json: Vec<u8>,
    pub identity: ContentIdentity,
}

pub fn build_comparison_envelope(
    vault: &VaultLease,
    operation_id: &str,
    knowledge_revision: u32,
    ranges: &[EvidenceRange],
) -> Result<PreparedComparison, String> {
    if ranges.is_empty() || ranges.len() > MAX_EVIDENCE_RANGES {
        return Err("Comparison requires 1 to 16 evidence ranges".to_owned());
    }
    let document = open_committed_revision(vault, operation_id, knowledge_revision)?;
    let markdown_identity = document
        .markdown_identity
        .ok_or_else(|| "Committed Markdown identity is missing".to_owned())?;
    let lines = exact_lines(&document.markdown);
    let mut range_ids = HashSet::with_capacity(ranges.len());
    let mut evidence = Vec::with_capacity(ranges.len());
    for range in ranges {
        if range.start_line == 0
            || range.end_line < range.start_line
            || range.end_line as usize > lines.len()
        {
            return Err("Evidence range is outside the committed Markdown".to_owned());
        }
        let evidence_id = format!("line-{}-{}", range.start_line, range.end_line);
        if !range_ids.insert(evidence_id.clone()) {
            return Err("Evidence ranges must be unique".to_owned());
        }
        let start = range.start_line as usize - 1;
        let end = range.end_line as usize;
        evidence.push(EvidenceExcerpt {
            evidence_id,
            start_line: range.start_line,
            end_line: range.end_line,
            text: lines[start..end].concat(),
        });
    }

    let rule_identity =
        ContentIdentity::from_reader(Cursor::new(KNOWLEDGE_RELATION_RULE_JSON.as_bytes()))
            .map_err(|error| format!("Knowledge relation rule cannot be hashed: {error}"))?;
    let envelope = ComparisonEnvelope {
        schema_version: COMPARISON_SCHEMA_VERSION,
        task: ComparisonTask::KnowledgeRelations,
        original_identity: document.original_identity,
        markdown_identity,
        knowledge_revision,
        rule_snapshot: RuleSnapshot {
            policy_id: "knowledge-relations-v1".to_owned(),
            version: "1.0.0".to_owned(),
            identity: rule_identity,
            json: KNOWLEDGE_RELATION_RULE_JSON.to_owned(),
        },
        evidence,
    };
    let json = serde_json::to_vec(&envelope)
        .map_err(|error| format!("Comparison envelope cannot be serialized: {error}"))?;
    if json.len() > MAX_ENVELOPE_BYTES {
        return Err("Comparison envelope exceeds 128 KiB".to_owned());
    }
    let identity = ContentIdentity::from_reader(Cursor::new(&json))
        .map_err(|error| format!("Comparison envelope cannot be hashed: {error}"))?;
    Ok(PreparedComparison {
        envelope,
        json,
        identity,
    })
}

pub fn persist_comparison_record(
    vault: &VaultLease,
    record: &ComparisonRecord,
) -> Result<(), String> {
    validate_comparison_record(record)?;
    let root = Path::new(".aiks/comparisons");
    ensure_trusted_directory(vault, root)?;
    if inspect_comparison_records(vault)?.len() >= MAX_COMPARISON_RECORDS {
        return Err("Comparison record namespace reached its storage limit".to_owned());
    }
    let namespace = root.join(&record.comparison_id);
    match vault.directory.symlink_metadata(&namespace) {
        Ok(_) => return Err("Comparison record already exists".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Comparison namespace cannot be inspected: {error}")),
    }
    vault
        .directory
        .create_dir(&namespace)
        .map_err(|error| format!("Comparison namespace cannot be created: {error}"))?;
    let result = write_new_json(&vault.directory, &namespace.join("00000001.json"), record);
    if result.is_err() {
        let _ = vault.directory.remove_dir(&namespace);
    }
    result
}

pub fn inspect_comparison_records(vault: &VaultLease) -> Result<Vec<ComparisonRecord>, String> {
    let root = Path::new(".aiks/comparisons");
    ensure_trusted_directory(vault, root)?;
    let mut records = Vec::new();
    for entry in vault
        .directory
        .read_dir(root)
        .map_err(|error| format!("Comparison records cannot be listed: {error}"))?
    {
        if records.len() >= MAX_COMPARISON_RECORDS {
            return Err("Comparison record namespace exceeds its scan limit".to_owned());
        }
        let entry = entry.map_err(|error| format!("Comparison entry cannot be read: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Comparison entry type cannot be read: {error}"))?;
        if file_type.is_symlink() || !file_type.is_dir() {
            return Err("Comparison namespace contains an untrusted entry".to_owned());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Comparison ID is not valid UTF-8".to_owned())?;
        validate_comparison_id(&name)?;
        let record: ComparisonRecord =
            read_json(&vault.directory, &root.join(&name).join("00000001.json"))?;
        validate_comparison_record(&record)?;
        if record.comparison_id != name {
            return Err("Comparison record does not match its namespace".to_owned());
        }
        records.push(record);
    }
    records.sort_by(|left, right| {
        right
            .recorded_at_unix_ms
            .cmp(&left.recorded_at_unix_ms)
            .then_with(|| left.comparison_id.cmp(&right.comparison_id))
    });
    Ok(records)
}

fn validate_comparison_record(record: &ComparisonRecord) -> Result<(), String> {
    if record.schema_version != COMPARISON_SCHEMA_VERSION {
        return Err("Comparison record schema version is unsupported".to_owned());
    }
    validate_comparison_id(&record.comparison_id)?;
    if record.desktop_config_id.is_empty()
        || record.agent_config_id.is_empty()
        || record.desktop_config_id == record.agent_config_id
        || record.actor != "desktop-orchestrator"
    {
        return Err("Comparison record authority metadata is invalid".to_owned());
    }
    let envelope_json = serde_json::to_vec(&record.envelope)
        .map_err(|error| format!("Comparison envelope cannot be serialized: {error}"))?;
    if envelope_json.len() > MAX_ENVELOPE_BYTES {
        return Err("Comparison envelope exceeds 128 KiB".to_owned());
    }
    let envelope_identity = ContentIdentity::from_reader(Cursor::new(envelope_json))
        .map_err(|error| format!("Comparison envelope cannot be hashed: {error}"))?;
    if record.envelope_identity != envelope_identity {
        return Err("Comparison envelope identity does not match its bytes".to_owned());
    }
    validate_envelope(&record.envelope)?;
    record.desktop_outcome.validate(&record.envelope)?;
    record.agent_outcome.validate(&record.envelope)?;
    if record.adjudication.is_some() && record.adjudication_failure.is_some() {
        return Err("Comparison cannot contain both adjudication and failure".to_owned());
    }
    if let Some(failure) = &record.adjudication_failure {
        if failure.is_empty()
            || failure.trim() != failure
            || failure.chars().count() > 2_048
            || failure.chars().any(char::is_control)
        {
            return Err("Adjudication failure is invalid".to_owned());
        }
    }
    if let Some(adjudication) = &record.adjudication {
        adjudication.validate(&record.envelope)?;
        if (adjudication.decision == super::protocol::AgentDecision::Review
            && record.status != ComparisonStatus::Review)
            || (adjudication.decision != super::protocol::AgentDecision::Review
                && record.status == ComparisonStatus::Review
                && record.adjudication_failure.is_none())
        {
            return Err("Comparison status does not match Agent adjudication".to_owned());
        }
    }
    match record.status {
        ComparisonStatus::Completed
            if record.adjudication.is_none()
                || record.adjudication_failure.is_some()
                || record.desktop_outcome.status != ProviderOutcomeStatus::Succeeded
                || record.agent_outcome.status != ProviderOutcomeStatus::Succeeded =>
        {
            Err("Completed comparison requires two proposals and Agent adjudication".to_owned())
        }
        ComparisonStatus::Failed
            if record.desktop_outcome.status != ProviderOutcomeStatus::Failed
                && record.agent_outcome.status != ProviderOutcomeStatus::Failed =>
        {
            Err("Failed comparison requires a provider failure".to_owned())
        }
        ComparisonStatus::Review
            if record.adjudication.is_none() && record.adjudication_failure.is_none() =>
        {
            Err("Review comparison requires an Agent review or adjudication failure".to_owned())
        }
        _ => Ok(()),
    }
}

fn validate_envelope(envelope: &ComparisonEnvelope) -> Result<(), String> {
    if envelope.schema_version != COMPARISON_SCHEMA_VERSION
        || envelope.knowledge_revision == 0
        || envelope.evidence.is_empty()
        || envelope.evidence.len() > MAX_EVIDENCE_RANGES
        || envelope.rule_snapshot.policy_id != "knowledge-relations-v1"
        || envelope.rule_snapshot.version != "1.0.0"
        || envelope.rule_snapshot.json != KNOWLEDGE_RELATION_RULE_JSON
    {
        return Err("Comparison envelope metadata is invalid".to_owned());
    }
    envelope.original_identity.validate()?;
    envelope.markdown_identity.validate()?;
    let rule_identity =
        ContentIdentity::from_reader(Cursor::new(KNOWLEDGE_RELATION_RULE_JSON.as_bytes()))
            .map_err(|error| format!("Knowledge relation rule cannot be hashed: {error}"))?;
    if envelope.rule_snapshot.identity != rule_identity {
        return Err("Comparison rule identity does not match its bytes".to_owned());
    }
    let mut evidence_ids = HashSet::with_capacity(envelope.evidence.len());
    for evidence in &envelope.evidence {
        if evidence.start_line == 0
            || evidence.end_line < evidence.start_line
            || evidence.evidence_id != format!("line-{}-{}", evidence.start_line, evidence.end_line)
            || !evidence_ids.insert(evidence.evidence_id.as_str())
        {
            return Err("Comparison envelope evidence metadata is invalid".to_owned());
        }
    }
    Ok(())
}

fn validate_comparison_id(value: &str) -> Result<(), String> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Comparison ID must be 32 lowercase hexadecimal characters".to_owned());
    }
    Ok(())
}

fn exact_lines(markdown: &str) -> Vec<&str> {
    let mut lines = markdown.split_inclusive('\n').collect::<Vec<_>>();
    if markdown.is_empty() || markdown.ends_with('\n') {
        lines.push("");
    }
    lines
}

fn ensure_trusted_directory(vault: &VaultLease, path: &Path) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Comparison storage path is not a trusted directory".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => vault
            .directory
            .create_dir(path)
            .map_err(|error| format!("Comparison storage directory cannot be created: {error}")),
        Err(error) => Err(format!(
            "Comparison storage directory cannot be inspected: {error}"
        )),
    }
}

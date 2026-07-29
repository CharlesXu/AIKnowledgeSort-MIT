use super::{
    classification_proposal, comparison_advice, requested_scope_id, ClassificationRequest,
    ComparisonRequest,
};
use crate::archive::{commit_plan_with_faults, ArchivePlan, ArchivePlanItem, TransactionFaults};
use crate::identity::ContentIdentity;
use crate::knowledge::save_document;
use crate::model_runtime::{
    AgentAdjudication, AgentDecision, EvidenceRange, ModelProposal, ProposalSide,
    RelationSuggestion,
};
use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
use crate::profiles::proposal::EvidenceReference;
use crate::profiles::schema::EvidenceKind;
use crate::profiles::{ProfileAuthority, ProfileDecision};
use crate::vault::{VaultAuthorityRegistry, VaultLease};
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const SOURCE_BYTES: &[u8] = b"governed MCP source bytes\n";

struct TestVault {
    root: PathBuf,
    registry: Option<VaultAuthorityRegistry>,
}

impl TestVault {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aiks-mcp-semantic-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).expect("create MCP semantic fixture");
        let root = root.canonicalize().expect("canonical MCP semantic fixture");
        let registry = VaultAuthorityRegistry::default();
        registry
            .authorize_path(&root)
            .expect("authorize fixture Vault");
        Self {
            root,
            registry: Some(registry),
        }
    }

    fn lease(&self) -> VaultLease {
        let registry = self.registry.as_ref().expect("fixture registry");
        let summary = registry.current_summary().expect("current fixture Vault");
        registry
            .lease(&summary.authority_id)
            .expect("lease fixture Vault")
    }

    fn granted_directory(&self) -> cap_std::fs::Dir {
        self.lease()
            .directory
            .try_clone()
            .expect("clone granted Vault scope")
    }
}

impl Drop for TestVault {
    fn drop(&mut self) {
        drop(self.registry.take());
        fs::remove_dir_all(&self.root).expect("remove MCP semantic fixture");
    }
}

fn approved_profile(authority: &ProfileAuthority, vault: &VaultLease) {
    let bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "schemaVersion": 1,
        "profileId": "mcp-fixture",
        "version": "1.0.0",
        "title": "MCP fixture",
        "status": "candidate",
        "provenance": {
            "sourceTitle": "Owned fixture",
            "ownership": "owned",
            "evidence": ["authorization:test"]
        },
        "rules": [{
            "ruleId": "fixture.quarterly-report",
            "destination": ["01-Research", "Reports"],
            "allOf": [{
                "kind": "documentText",
                "term": "quarterly report"
            }]
        }]
    }))
    .expect("serialize candidate profile");
    let candidate = authority
        .import_local_bytes(
            vault,
            "mcp-fixture.json",
            "/owned/mcp-fixture.json",
            &bytes,
            SystemTime::UNIX_EPOCH,
        )
        .expect("import profile candidate");
    authority
        .decide(
            vault,
            &candidate.candidate_id,
            &candidate.source_identity.digest,
            ProfileDecision::Approve,
            SystemTime::UNIX_EPOCH,
        )
        .expect("approve profile candidate");
}

#[test]
fn extracts_scope_from_semantic_tool_arguments_without_rejecting_tool_fields() {
    let arguments = serde_json::json!({
        "scopeId": "vault-scope",
        "sourceIdentity": {
            "algorithm": "SHA-256",
            "digest": source_identity().digest
        },
        "references": [{
            "kind": "documentText",
            "location": "page:1",
            "text": "Quarterly report"
        }]
    })
    .as_object()
    .cloned();

    assert_eq!(
        requested_scope_id("classification.propose", &arguments)
            .expect("extract classification scope"),
        Some("vault-scope".to_owned())
    );
}

fn source_identity() -> ContentIdentity {
    ContentIdentity::from_reader(Cursor::new(SOURCE_BYTES)).expect("hash source fixture")
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(base: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = directory
            .read_dir()
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let metadata = entry.metadata().expect("read snapshot metadata");
            if metadata.is_dir() {
                visit(base, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(base)
                        .expect("snapshot relative path")
                        .to_path_buf(),
                    fs::read(path).expect("read snapshot file"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn proposes_from_the_exact_approved_profile_without_mutating_the_vault() {
    let vault = TestVault::new();
    let profiles = ProfileAuthority::default();
    approved_profile(&profiles, &vault.lease());
    let before = snapshot(&vault.root.join(".aiks"));

    let result = classification_proposal(
        &profiles,
        vault.granted_directory(),
        ClassificationRequest {
            scope_id: "vault-scope".to_owned(),
            source_identity: source_identity(),
            references: vec![EvidenceReference {
                kind: EvidenceKind::DocumentText,
                location: "page:3".to_owned(),
                text: "Quarterly report for Project Atlas".to_owned(),
            }],
        },
    )
    .expect("propose classification");

    assert_eq!(result["executionAvailable"], false);
    assert_eq!(result["requiresDesktopReview"], true);
    assert_eq!(result["proposal"]["profileId"], "mcp-fixture");
    assert_eq!(result["proposal"]["profileVersion"], "1.0.0");
    assert_eq!(result["proposal"]["status"], "proposed");
    assert_eq!(
        result["proposal"]["destination"],
        serde_json::json!(["01-Research", "Reports"])
    );
    assert_eq!(result["proposal"]["committable"], true);
    assert_eq!(snapshot(&vault.root.join(".aiks")), before);
}

#[test]
fn returns_review_for_missing_semantic_evidence_without_mutation() {
    let vault = TestVault::new();
    let profiles = ProfileAuthority::default();
    approved_profile(&profiles, &vault.lease());
    let before = snapshot(&vault.root.join(".aiks"));

    let result = classification_proposal(
        &profiles,
        vault.granted_directory(),
        ClassificationRequest {
            scope_id: "vault-scope".to_owned(),
            source_identity: source_identity(),
            references: vec![EvidenceReference {
                kind: EvidenceKind::DocumentText,
                location: "page:1".to_owned(),
                text: "Unrelated material".to_owned(),
            }],
        },
    )
    .expect("return classification review");

    assert_eq!(result["proposal"]["status"], "classificationReview");
    assert_eq!(result["proposal"]["reviewReason"], "missingEvidence");
    assert_eq!(result["proposal"]["committable"], false);
    assert_eq!(snapshot(&vault.root.join(".aiks")), before);
}

fn committed_knowledge(vault: &TestVault) -> String {
    let source = vault.root.join("source.txt");
    fs::write(&source, SOURCE_BYTES).expect("write source fixture");
    let lease = vault.lease();
    let identity = source_identity();
    let destination_path = format!("Originals/{}/MCP-source.txt", identity.digest);
    let plan = ArchivePlan {
        plan_id: "mcp-comparison-plan".to_owned(),
        plan_version: 2,
        proposal_id: "mcp-comparison-proposal".to_owned(),
        naming_batch_id: "mcp-comparison-naming".to_owned(),
        classification_batch_id: None,
        authority_id: lease.summary.authority_id.clone(),
        vault_path: vault.root.to_string_lossy().into_owned(),
        expires_at_unix_ms: u64::MAX,
        confirmation_nonce: "mcp-comparison-confirmation".to_owned(),
        source_preserved: true,
        items: vec![ArchivePlanItem {
            item_id: "mcp-comparison-item".to_owned(),
            source_path: source.to_string_lossy().into_owned(),
            destination_path,
            original_name: "source.txt".to_owned(),
            canonical_name: "MCP-source.txt".to_owned(),
            classification: None,
            naming: NamingDecisionEvidence {
                naming_proposal_id: "mcp-comparison-name".to_owned(),
                original_name: "source.txt".to_owned(),
                canonical_name: "MCP-source.txt".to_owned(),
                policy_id: "canonical-v1".to_owned(),
                policy_version: "1.0.0".to_owned(),
                applied_rule: "ordered-cited-facts-v1".to_owned(),
                facts: vec![NamingFact {
                    kind: NamingFactKind::Subject,
                    value: "MCP source".to_owned(),
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
        "# MCP source\nEvidence alpha connects Project Atlas to quarterly reporting.\n",
    )
    .expect("save committed knowledge");
    operation_id
}

fn proposal(summary: &str, evidence_id: &str) -> ModelProposal {
    ModelProposal {
        summary: summary.to_owned(),
        relations: vec![RelationSuggestion {
            source: "Project Atlas".to_owned(),
            relation_type: "documentedBy".to_owned(),
            target: "Quarterly report".to_owned(),
            evidence_ids: vec![evidence_id.to_owned()],
        }],
    }
}

#[test]
fn validates_two_outputs_and_agent_adjudication_without_persisting_results() {
    let vault = TestVault::new();
    let operation_id = committed_knowledge(&vault);
    let before = snapshot(&vault.root.join(".aiks"));

    let result = comparison_advice(
        vault.granted_directory(),
        ComparisonRequest {
            scope_id: "vault-scope".to_owned(),
            operation_id,
            knowledge_revision: 1,
            evidence_ranges: vec![EvidenceRange {
                start_line: 2,
                end_line: 2,
            }],
            desktop_proposal: proposal("Desktop output", "line-2-2"),
            agent_proposal: proposal("Agent output", "line-2-2"),
            adjudication: AgentAdjudication {
                decision: AgentDecision::Accept,
                reason: "Agent output is supported by the cited revision.".to_owned(),
                evidence_ids: vec!["line-2-2".to_owned()],
                selected_side: Some(ProposalSide::Agent),
                revised_relations: Vec::new(),
            },
        },
    )
    .expect("validate comparison advice");

    assert_eq!(result["executionAvailable"], false);
    assert_eq!(result["requiresDesktopGraphReview"], true);
    assert_eq!(result["status"], "completed");
    assert_eq!(result["adjudication"]["selectedSide"], "agent");
    assert_eq!(result["envelope"]["knowledgeRevision"], 1);
    assert_eq!(snapshot(&vault.root.join(".aiks")), before);
}

#[test]
fn rejects_invented_evidence_ids_without_persisting_results() {
    let vault = TestVault::new();
    let operation_id = committed_knowledge(&vault);
    let before = snapshot(&vault.root.join(".aiks"));

    let error = comparison_advice(
        vault.granted_directory(),
        ComparisonRequest {
            scope_id: "vault-scope".to_owned(),
            operation_id,
            knowledge_revision: 1,
            evidence_ranges: vec![EvidenceRange {
                start_line: 2,
                end_line: 2,
            }],
            desktop_proposal: proposal("Desktop output", "invented-evidence"),
            agent_proposal: proposal("Agent output", "line-2-2"),
            adjudication: AgentAdjudication {
                decision: AgentDecision::Review,
                reason: "The outputs require review.".to_owned(),
                evidence_ids: vec!["line-2-2".to_owned()],
                selected_side: None,
                revised_relations: Vec::new(),
            },
        },
    )
    .expect_err("reject invented evidence");

    assert_eq!(error, "Semantic output contains an invalid evidence ID");
    assert_eq!(snapshot(&vault.root.join(".aiks")), before);
}

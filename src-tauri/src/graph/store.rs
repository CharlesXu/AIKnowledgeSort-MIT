use crate::archive::verified_registered_original;
use crate::identity::ContentIdentity;
use crate::knowledge::open_committed_revision;
use crate::vault::records::{read_json, write_new_json};
use crate::vault::VaultLease;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const GRAPH_SCHEMA_VERSION: u32 = 1;
const MAX_RELATIONS: usize = 10_000;
const MAX_RELATION_VERSIONS: u32 = 100;
const MAX_EVIDENCE_RANGES: usize = 16;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRange {
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationRevisionInput {
    pub knowledge_revision: u32,
    pub source_node: String,
    pub relation_type: String,
    pub target_node: String,
    pub evidence_ranges: Vec<EvidenceRange>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphDecision {
    Accept,
    Revise,
    Reject,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationStatus {
    Review,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceReference {
    pub operation_id: String,
    pub knowledge_revision: u32,
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
    pub markdown_identity: ContentIdentity,
    pub original_identity: ContentIdentity,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphRelation {
    schema_version: u32,
    pub relation_id: String,
    pub version: u32,
    pub authority_id: String,
    pub operation_id: String,
    pub knowledge_revision: u32,
    pub source_node: String,
    pub relation_type: String,
    pub target_node: String,
    pub status: RelationStatus,
    pub evidence: Vec<EvidenceReference>,
    pub actor: String,
    pub reason: String,
    pub recorded_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvent {
    pub relation_id: String,
    pub version: u32,
    pub status: RelationStatus,
    pub source_node: String,
    pub relation_type: String,
    pub target_node: String,
    pub recorded_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphSnapshot {
    pub authority_id: String,
    pub operation_id: String,
    pub relations: Vec<GraphRelation>,
    pub events: Vec<GraphEvent>,
}

pub(crate) fn propose_relation(
    vault: &VaultLease,
    operation_id: &str,
    knowledge_revision: u32,
    source_node: &str,
    relation_type: &str,
    target_node: &str,
    evidence_ranges: &[EvidenceRange],
) -> Result<GraphRelation, String> {
    let relation_id = Uuid::new_v4().simple().to_string();
    let relation = build_relation(
        vault,
        &relation_id,
        1,
        operation_id,
        knowledge_revision,
        source_node,
        relation_type,
        target_node,
        evidence_ranges,
        RelationStatus::Review,
        "Proposed from verified Markdown evidence",
    )?;
    let directory = relation_directory(&relation_id);
    ensure_trusted_directory(vault, &directory)?;
    write_relation(vault, &relation)?;
    Ok(relation)
}

pub(crate) fn decide_relation(
    vault: &VaultLease,
    relation_id: &str,
    expected_version: u32,
    decision: GraphDecision,
    reason: &str,
    revision: Option<&RelationRevisionInput>,
) -> Result<GraphRelation, String> {
    validate_relation_id(relation_id)?;
    let current = latest_relation(vault, relation_id)?;
    if current.version != expected_version {
        return Err("Graph relation version changed; inspect before deciding".to_owned());
    }
    if current.status != RelationStatus::Review {
        return Err("A terminal graph relation cannot be decided again".to_owned());
    }
    let reason = bounded_text(reason, 512, "Graph decision reason")?;
    let version = current
        .version
        .checked_add(1)
        .filter(|value| *value <= MAX_RELATION_VERSIONS)
        .ok_or_else(|| "Graph relation reached its version limit".to_owned())?;

    let next = match decision {
        GraphDecision::Accept | GraphDecision::Reject => {
            if revision.is_some() {
                return Err("Accept and reject cannot include a revision payload".to_owned());
            }
            verify_relation_evidence(vault, &current)?;
            GraphRelation {
                version,
                status: if decision == GraphDecision::Accept {
                    RelationStatus::Accepted
                } else {
                    RelationStatus::Rejected
                },
                actor: "desktop-user".to_owned(),
                reason,
                recorded_at_unix_ms: unix_time_ms(),
                ..current
            }
        }
        GraphDecision::Revise => {
            let revision =
                revision.ok_or_else(|| "Revise requires replacement relation fields".to_owned())?;
            build_relation(
                vault,
                relation_id,
                version,
                &current.operation_id,
                revision.knowledge_revision,
                &revision.source_node,
                &revision.relation_type,
                &revision.target_node,
                &revision.evidence_ranges,
                RelationStatus::Review,
                &reason,
            )?
        }
    };
    write_relation(vault, &next)?;
    Ok(next)
}

pub(crate) fn inspect_graph(
    vault: &VaultLease,
    operation_id: &str,
) -> Result<GraphSnapshot, String> {
    let original = verified_registered_original(vault, operation_id)?;
    let base = Path::new(".aiks/graph/relations");
    let mut relations = Vec::new();
    let mut events = Vec::new();
    for (index, entry) in vault
        .directory
        .read_dir(base)
        .map_err(|error| format!("Graph relation store cannot be listed: {error}"))?
        .enumerate()
    {
        if index >= MAX_RELATIONS {
            return Err("Graph relation store exceeds its scan limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Graph relation entry is unreadable: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Graph relation entry type is unreadable: {error}"))?;
        if file_type.is_symlink() || !file_type.is_dir() {
            return Err("Graph relation store contains an untrusted entry".to_owned());
        }
        let relation_id = entry
            .file_name()
            .into_string()
            .map_err(|_| "Graph relation ID is not valid UTF-8".to_owned())?;
        validate_relation_id(&relation_id)?;
        let versions = relation_versions(vault, &relation_id)?;
        if versions.iter().any(|relation| {
            relation.operation_id == operation_id
                && relation
                    .evidence
                    .iter()
                    .any(|evidence| evidence.original_identity != original.identity)
        }) {
            return Err("Graph relation provenance no longer matches its original".to_owned());
        }
        let matching = versions
            .into_iter()
            .filter(|relation| relation.operation_id == operation_id)
            .collect::<Vec<_>>();
        if let Some(latest) = matching.last() {
            verify_relation_evidence(vault, latest)?;
            relations.push(latest.clone());
        }
        events.extend(matching.into_iter().map(|relation| GraphEvent {
            relation_id: relation.relation_id,
            version: relation.version,
            status: relation.status,
            source_node: relation.source_node,
            relation_type: relation.relation_type,
            target_node: relation.target_node,
            recorded_at_unix_ms: relation.recorded_at_unix_ms,
        }));
    }
    relations.sort_by(|left, right| left.relation_id.cmp(&right.relation_id));
    events.sort_by(|left, right| {
        (left.recorded_at_unix_ms, &left.relation_id, left.version).cmp(&(
            right.recorded_at_unix_ms,
            &right.relation_id,
            right.version,
        ))
    });
    Ok(GraphSnapshot {
        authority_id: vault.summary.authority_id.clone(),
        operation_id: operation_id.to_owned(),
        relations,
        events,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_relation(
    vault: &VaultLease,
    relation_id: &str,
    version: u32,
    operation_id: &str,
    knowledge_revision: u32,
    source_node: &str,
    relation_type: &str,
    target_node: &str,
    evidence_ranges: &[EvidenceRange],
    status: RelationStatus,
    reason: &str,
) -> Result<GraphRelation, String> {
    let source_node = bounded_text(source_node, 160, "Graph source node")?;
    let relation_type = bounded_text(relation_type, 80, "Graph relation type")?;
    let target_node = bounded_text(target_node, 160, "Graph target node")?;
    if source_node == target_node {
        return Err("Graph relation source and target must differ".to_owned());
    }
    let document = open_committed_revision(vault, operation_id, knowledge_revision)?;
    let markdown_identity = document
        .markdown_identity
        .clone()
        .ok_or_else(|| "Committed Markdown identity is missing".to_owned())?;
    let evidence = extract_evidence(
        operation_id,
        knowledge_revision,
        &document.markdown,
        &markdown_identity,
        &document.original_identity,
        evidence_ranges,
    )?;
    Ok(GraphRelation {
        schema_version: GRAPH_SCHEMA_VERSION,
        relation_id: relation_id.to_owned(),
        version,
        authority_id: vault.summary.authority_id.clone(),
        operation_id: operation_id.to_owned(),
        knowledge_revision,
        source_node,
        relation_type,
        target_node,
        status,
        evidence,
        actor: "desktop-user".to_owned(),
        reason: bounded_text(reason, 512, "Graph relation reason")?,
        recorded_at_unix_ms: unix_time_ms(),
    })
}

fn extract_evidence(
    operation_id: &str,
    knowledge_revision: u32,
    markdown: &str,
    markdown_identity: &ContentIdentity,
    original_identity: &ContentIdentity,
    ranges: &[EvidenceRange],
) -> Result<Vec<EvidenceReference>, String> {
    if ranges.is_empty() || ranges.len() > MAX_EVIDENCE_RANGES {
        return Err("Graph relation requires 1 to 16 evidence ranges".to_owned());
    }
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut seen = Vec::<(u32, u32)>::new();
    let mut evidence = Vec::with_capacity(ranges.len());
    for range in ranges {
        if range.start_line == 0
            || range.end_line < range.start_line
            || range.end_line as usize > lines.len()
            || seen.contains(&(range.start_line, range.end_line))
        {
            return Err("Graph evidence range is invalid or duplicated".to_owned());
        }
        seen.push((range.start_line, range.end_line));
        let text = lines[(range.start_line - 1) as usize..range.end_line as usize].join("\n");
        if text.trim().is_empty() {
            return Err("Graph evidence range contains no claim text".to_owned());
        }
        evidence.push(EvidenceReference {
            operation_id: operation_id.to_owned(),
            knowledge_revision,
            start_line: range.start_line,
            end_line: range.end_line,
            text,
            markdown_identity: markdown_identity.clone(),
            original_identity: original_identity.clone(),
        });
    }
    Ok(evidence)
}

fn verify_relation_evidence(vault: &VaultLease, relation: &GraphRelation) -> Result<(), String> {
    let ranges = relation
        .evidence
        .iter()
        .map(|evidence| EvidenceRange {
            start_line: evidence.start_line,
            end_line: evidence.end_line,
        })
        .collect::<Vec<_>>();
    let document =
        open_committed_revision(vault, &relation.operation_id, relation.knowledge_revision)?;
    let extracted = extract_evidence(
        &relation.operation_id,
        relation.knowledge_revision,
        &document.markdown,
        document
            .markdown_identity
            .as_ref()
            .ok_or_else(|| "Committed Markdown identity is missing".to_owned())?,
        &document.original_identity,
        &ranges,
    )?;
    if extracted != relation.evidence {
        return Err("Graph relation evidence failed current verification".to_owned());
    }
    Ok(())
}

fn write_relation(vault: &VaultLease, relation: &GraphRelation) -> Result<(), String> {
    let path =
        relation_directory(&relation.relation_id).join(format!("{:08}.json", relation.version));
    write_new_json(&vault.directory, &path, relation)
}

fn latest_relation(vault: &VaultLease, relation_id: &str) -> Result<GraphRelation, String> {
    relation_versions(vault, relation_id)?
        .pop()
        .ok_or_else(|| "Graph relation was not found".to_owned())
}

fn relation_versions(vault: &VaultLease, relation_id: &str) -> Result<Vec<GraphRelation>, String> {
    validate_relation_id(relation_id)?;
    let directory = relation_directory(relation_id);
    let mut versions = Vec::new();
    for (index, entry) in vault
        .directory
        .read_dir(&directory)
        .map_err(|error| format!("Graph relation versions cannot be listed: {error}"))?
        .enumerate()
    {
        if index >= MAX_RELATION_VERSIONS as usize {
            return Err("Graph relation exceeds its version scan limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Graph relation version is unreadable: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Graph relation version type is unreadable: {error}"))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err("Graph relation contains an untrusted version entry".to_owned());
        }
        let path = directory.join(entry.file_name());
        let relation: GraphRelation = read_json(&vault.directory, &path)?;
        validate_relation_record(vault, relation_id, &relation)?;
        if path.file_name().and_then(|value| value.to_str())
            != Some(format!("{:08}.json", relation.version).as_str())
        {
            return Err("Graph relation version filename is inconsistent".to_owned());
        }
        versions.push(relation);
    }
    versions.sort_by_key(|relation| relation.version);
    for (index, relation) in versions.iter().enumerate() {
        if relation.version != index as u32 + 1 {
            return Err("Graph relation version sequence is invalid".to_owned());
        }
    }
    Ok(versions)
}

fn validate_relation_record(
    vault: &VaultLease,
    relation_id: &str,
    relation: &GraphRelation,
) -> Result<(), String> {
    if relation.schema_version != GRAPH_SCHEMA_VERSION
        || relation.relation_id != relation_id
        || relation.authority_id != vault.summary.authority_id
        || relation.version == 0
        || relation.version > MAX_RELATION_VERSIONS
        || relation.evidence.is_empty()
    {
        return Err("Graph relation record is invalid".to_owned());
    }
    for evidence in &relation.evidence {
        evidence.markdown_identity.validate()?;
        evidence.original_identity.validate()?;
        if evidence.operation_id != relation.operation_id
            || evidence.knowledge_revision != relation.knowledge_revision
        {
            return Err("Graph relation evidence provenance is inconsistent".to_owned());
        }
    }
    Ok(())
}

fn ensure_trusted_directory(vault: &VaultLease, path: &Path) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Graph storage path is not a trusted directory".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => vault
            .directory
            .create_dir(path)
            .map_err(|error| format!("Graph storage directory cannot be created: {error}")),
        Err(error) => Err(format!(
            "Graph storage directory cannot be inspected: {error}"
        )),
    }
}

fn relation_directory(relation_id: &str) -> std::path::PathBuf {
    Path::new(".aiks/graph/relations").join(relation_id)
}

fn validate_relation_id(value: &str) -> Result<(), String> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Graph relation ID is invalid".to_owned());
    }
    Ok(())
}

fn bounded_text(value: &str, max_chars: usize, label: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{label} is empty, oversized, or contains control characters"
        ));
    }
    Ok(value.to_owned())
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        decide_relation, inspect_graph, propose_relation, EvidenceRange, GraphDecision,
        RelationRevisionInput, RelationStatus,
    };
    use crate::archive::{
        commit_plan_with_faults, ArchivePlan, ArchivePlanItem, TransactionFaults,
    };
    use crate::identity::ContentIdentity;
    use crate::knowledge::save_document;
    use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
    use crate::vault::VaultAuthorityRegistry;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SOURCE_BYTES: &[u8] = b"graph source bytes\n";
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> (PathBuf, PathBuf, crate::vault::VaultLease, String) {
        let root = std::env::temp_dir().join(format!(
            "aiks-graph-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("source.txt");
        fs::write(&source, SOURCE_BYTES).unwrap();
        let vault_path = root.join("vault");
        fs::create_dir(&vault_path).unwrap();
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults.authorize_path(&vault_path).unwrap();
        let lease = vaults.lease(&summary.authority_id).unwrap();
        let identity = ContentIdentity::from_reader(Cursor::new(SOURCE_BYTES)).unwrap();
        let plan = ArchivePlan {
            plan_id: "graph-plan".to_owned(),
            plan_version: 2,
            proposal_id: "graph-proposal".to_owned(),
            naming_batch_id: "graph-naming".to_owned(),
            classification_batch_id: None,
            authority_id: summary.authority_id,
            vault_path: vault_path.to_string_lossy().into_owned(),
            expires_at_unix_ms: u64::MAX,
            confirmation_nonce: "graph-confirmation".to_owned(),
            source_preserved: true,
            items: vec![ArchivePlanItem {
                item_id: "graph-item".to_owned(),
                source_path: source.to_string_lossy().into_owned(),
                destination_path: format!("Originals/{}/Graph-source.txt", identity.digest),
                original_name: "source.txt".to_owned(),
                canonical_name: "Graph-source.txt".to_owned(),
                classification: None,
                naming: NamingDecisionEvidence {
                    naming_proposal_id: "graph-name".to_owned(),
                    original_name: "source.txt".to_owned(),
                    canonical_name: "Graph-source.txt".to_owned(),
                    policy_id: "canonical-v1".to_owned(),
                    policy_version: "1.0.0".to_owned(),
                    applied_rule: "ordered-cited-facts-v1".to_owned(),
                    facts: vec![NamingFact {
                        kind: NamingFactKind::Subject,
                        value: "Graph source".to_owned(),
                        evidence_location: "page:1".to_owned(),
                    }],
                },
                byte_size: SOURCE_BYTES.len() as u64,
                identity,
            }],
        };
        let result = commit_plan_with_faults(plan, &lease, TransactionFaults::default());
        let operation_id = result.items[0].operation_id.clone();
        save_document(
            &lease,
            &operation_id,
            0,
            "# Reset reliability\nBrown-out threshold evidence.\nClock stabilization evidence.\n",
        )
        .unwrap();
        (root, source, lease, operation_id)
    }

    #[test]
    fn persists_exact_evidence_and_non_replayable_review_versions() {
        let (root, source, lease, operation_id) = fixture();
        let proposed = propose_relation(
            &lease,
            &operation_id,
            1,
            "MCU reset",
            "depends on",
            "Brown-out threshold",
            &[EvidenceRange {
                start_line: 2,
                end_line: 3,
            }],
        )
        .unwrap();
        assert_eq!(proposed.version, 1);
        assert_eq!(proposed.status, RelationStatus::Review);
        assert_eq!(
            proposed.evidence[0].text,
            "Brown-out threshold evidence.\nClock stabilization evidence."
        );
        assert_eq!(proposed.evidence[0].operation_id, operation_id);
        assert_eq!(proposed.evidence[0].knowledge_revision, 1);
        assert_eq!(proposed.actor, "desktop-user");

        let revised = decide_relation(
            &lease,
            &proposed.relation_id,
            1,
            GraphDecision::Revise,
            "Narrow the claim",
            Some(&RelationRevisionInput {
                knowledge_revision: 1,
                source_node: "MCU reset".to_owned(),
                relation_type: "requires".to_owned(),
                target_node: "Clock stabilization".to_owned(),
                evidence_ranges: vec![EvidenceRange {
                    start_line: 3,
                    end_line: 3,
                }],
            }),
        )
        .unwrap();
        assert_eq!(revised.relation_id, proposed.relation_id);
        assert_eq!(revised.version, 2);
        assert_eq!(revised.status, RelationStatus::Review);

        let accepted = decide_relation(
            &lease,
            &revised.relation_id,
            2,
            GraphDecision::Accept,
            "Evidence verified",
            None,
        )
        .unwrap();
        assert_eq!(accepted.version, 3);
        assert_eq!(accepted.status, RelationStatus::Accepted);
        assert!(decide_relation(
            &lease,
            &accepted.relation_id,
            3,
            GraphDecision::Reject,
            "Replay",
            None,
        )
        .is_err());

        let snapshot = inspect_graph(&lease, &operation_id).unwrap();
        assert_eq!(snapshot.relations, vec![accepted]);
        assert_eq!(snapshot.events.len(), 3);
        assert_eq!(fs::read(source).unwrap(), SOURCE_BYTES);
        drop(lease);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_invalid_and_unverifiable_evidence_without_mutation() {
        let (root, source, lease, operation_id) = fixture();
        for ranges in [
            vec![],
            vec![EvidenceRange {
                start_line: 0,
                end_line: 1,
            }],
            vec![EvidenceRange {
                start_line: 4,
                end_line: 4,
            }],
        ] {
            assert!(propose_relation(
                &lease,
                &operation_id,
                1,
                "Source",
                "relates to",
                "Target",
                &ranges,
            )
            .is_err());
        }
        assert!(propose_relation(
            &lease,
            &operation_id,
            2,
            "Source",
            "relates to",
            "Target",
            &[EvidenceRange {
                start_line: 1,
                end_line: 1,
            }],
        )
        .is_err());
        assert!(inspect_graph(&lease, &operation_id)
            .unwrap()
            .relations
            .is_empty());
        assert_eq!(fs::read(source).unwrap(), SOURCE_BYTES);
        drop(lease);
        fs::remove_dir_all(root).unwrap();
    }
}

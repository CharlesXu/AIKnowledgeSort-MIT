use crate::archive::verified_registered_original;
use crate::identity::ContentIdentity;
use crate::vault::records::{read_bytes_bounded, read_json, write_new_bytes, write_new_json};
use crate::vault::VaultLease;
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const KNOWLEDGE_SCHEMA_VERSION: u32 = 1;
const MAX_MARKDOWN_BYTES: usize = 1024 * 1024;
const MAX_REVISIONS: u32 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeDocument {
    pub document_id: String,
    pub authority_id: String,
    pub operation_id: String,
    pub revision: u32,
    pub markdown_path: Option<String>,
    pub markdown: String,
    pub saved_at_unix_ms: Option<u64>,
    pub markdown_identity: Option<ContentIdentity>,
    pub original_identity: ContentIdentity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KnowledgeRevisionRecord {
    schema_version: u32,
    document_id: String,
    authority_id: String,
    operation_id: String,
    revision: u32,
    markdown_path: String,
    saved_at_unix_ms: u64,
    markdown_identity: ContentIdentity,
    original_identity: ContentIdentity,
}

pub(crate) fn open_document(
    vault: &VaultLease,
    operation_id: &str,
) -> Result<KnowledgeDocument, String> {
    let original = verified_registered_original(vault, operation_id)?;
    match latest_revision(vault, operation_id)? {
        Some(record) => read_committed_revision(vault, &original.identity, record),
        None => Ok(KnowledgeDocument {
            document_id: operation_id.to_owned(),
            authority_id: original.authority_id,
            operation_id: original.operation_id,
            revision: 0,
            markdown_path: None,
            markdown: starter_markdown(
                &original.canonical_name,
                &original.relative_path,
                &original.identity,
            ),
            saved_at_unix_ms: None,
            markdown_identity: None,
            original_identity: original.identity,
        }),
    }
}

pub(crate) fn open_committed_revision(
    vault: &VaultLease,
    operation_id: &str,
    revision: u32,
) -> Result<KnowledgeDocument, String> {
    if revision == 0 || revision > MAX_REVISIONS {
        return Err("Committed knowledge revision is invalid".to_owned());
    }
    let original = verified_registered_original(vault, operation_id)?;
    let path = Path::new(".aiks/knowledge")
        .join(operation_id)
        .join(format!("{revision:08}.json"));
    let record: KnowledgeRevisionRecord = read_json(&vault.directory, &path)?;
    validate_record(vault, operation_id, &record)?;
    if record.revision != revision {
        return Err("Knowledge revision metadata does not match the request".to_owned());
    }
    read_committed_revision(vault, &original.identity, record)
}

pub(crate) fn save_document(
    vault: &VaultLease,
    operation_id: &str,
    expected_revision: u32,
    markdown: &str,
) -> Result<KnowledgeDocument, String> {
    if markdown.len() > MAX_MARKDOWN_BYTES {
        return Err("Markdown exceeds the 1 MiB document limit".to_owned());
    }
    let original = verified_registered_original(vault, operation_id)?;
    let current_revision = latest_revision(vault, operation_id)?
        .map(|record| record.revision)
        .unwrap_or(0);
    if current_revision != expected_revision {
        return Err("Knowledge document revision changed; reopen before saving".to_owned());
    }
    let revision = current_revision
        .checked_add(1)
        .filter(|value| *value <= MAX_REVISIONS)
        .ok_or_else(|| "Knowledge document reached its revision limit".to_owned())?;
    let record_directory = Path::new(".aiks/knowledge").join(operation_id);
    let markdown_directory = Path::new("Knowledge").join(operation_id);
    ensure_trusted_directory(vault, &record_directory)?;
    ensure_trusted_directory(vault, &markdown_directory)?;

    let revision_name = format!("{revision:08}");
    let markdown_path = markdown_directory.join(format!("{revision_name}.md"));
    let record_path = record_directory.join(format!("{revision_name}.json"));
    remove_uncommitted_markdown(vault, &markdown_path, &record_path)?;
    let markdown_identity = ContentIdentity::from_reader(Cursor::new(markdown.as_bytes()))
        .map_err(|error| format!("Markdown identity cannot be computed: {error}"))?;
    write_new_bytes(&vault.directory, &markdown_path, markdown.as_bytes())?;
    let record = KnowledgeRevisionRecord {
        schema_version: KNOWLEDGE_SCHEMA_VERSION,
        document_id: operation_id.to_owned(),
        authority_id: original.authority_id.clone(),
        operation_id: original.operation_id.clone(),
        revision,
        markdown_path: markdown_path.to_string_lossy().into_owned(),
        saved_at_unix_ms: unix_time_ms(),
        markdown_identity: markdown_identity.clone(),
        original_identity: original.identity.clone(),
    };
    write_new_json(&vault.directory, &record_path, &record)?;

    Ok(KnowledgeDocument {
        document_id: record.document_id,
        authority_id: record.authority_id,
        operation_id: record.operation_id,
        revision,
        markdown_path: Some(record.markdown_path),
        markdown: markdown.to_owned(),
        saved_at_unix_ms: Some(record.saved_at_unix_ms),
        markdown_identity: Some(markdown_identity),
        original_identity: record.original_identity,
    })
}

fn latest_revision(
    vault: &VaultLease,
    operation_id: &str,
) -> Result<Option<KnowledgeRevisionRecord>, String> {
    let directory = Path::new(".aiks/knowledge").join(operation_id);
    match vault.directory.symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("Knowledge revision namespace is not a trusted directory".to_owned())
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Knowledge revision namespace is unreadable: {error}"
            ))
        }
    }

    let mut latest: Option<KnowledgeRevisionRecord> = None;
    for (index, entry) in vault
        .directory
        .read_dir(&directory)
        .map_err(|error| format!("Knowledge revisions cannot be listed: {error}"))?
        .enumerate()
    {
        if index >= MAX_REVISIONS as usize {
            return Err("Knowledge revision namespace exceeds its scan limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Knowledge revision entry is unreadable: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Knowledge revision entry type is unreadable: {error}"))?;
        if file_type.is_symlink() {
            return Err("Knowledge revision namespace contains a link".to_owned());
        }
        if !file_type.is_file()
            || Path::new(&entry.file_name())
                .extension()
                .and_then(|value| value.to_str())
                != Some("json")
        {
            continue;
        }
        let path = directory.join(entry.file_name());
        let record: KnowledgeRevisionRecord = read_json(&vault.directory, &path)?;
        validate_record(vault, operation_id, &record)?;
        if latest
            .as_ref()
            .map_or(true, |current| record.revision > current.revision)
        {
            latest = Some(record);
        }
    }
    Ok(latest)
}

fn validate_record(
    vault: &VaultLease,
    operation_id: &str,
    record: &KnowledgeRevisionRecord,
) -> Result<(), String> {
    let expected_path = Path::new("Knowledge")
        .join(operation_id)
        .join(format!("{:08}.md", record.revision));
    if record.schema_version != KNOWLEDGE_SCHEMA_VERSION
        || record.document_id != operation_id
        || record.operation_id != operation_id
        || record.authority_id != vault.summary.authority_id
        || record.revision == 0
        || record.revision > MAX_REVISIONS
        || Path::new(&record.markdown_path) != expected_path
    {
        return Err("Knowledge revision metadata is invalid".to_owned());
    }
    record.markdown_identity.validate()?;
    record.original_identity.validate()
}

fn read_committed_revision(
    vault: &VaultLease,
    original_identity: &ContentIdentity,
    record: KnowledgeRevisionRecord,
) -> Result<KnowledgeDocument, String> {
    if &record.original_identity != original_identity {
        return Err("Knowledge provenance no longer matches the registered original".to_owned());
    }
    let bytes = read_bytes_bounded(
        &vault.directory,
        Path::new(&record.markdown_path),
        MAX_MARKDOWN_BYTES,
    )?;
    let identity = ContentIdentity::from_reader(Cursor::new(&bytes))
        .map_err(|error| format!("Markdown identity cannot be computed: {error}"))?;
    if identity != record.markdown_identity {
        return Err("Authoritative Markdown failed SHA-256 verification".to_owned());
    }
    let markdown = String::from_utf8(bytes)
        .map_err(|_| "Authoritative Markdown is not valid UTF-8".to_owned())?;
    Ok(KnowledgeDocument {
        document_id: record.document_id,
        authority_id: record.authority_id,
        operation_id: record.operation_id,
        revision: record.revision,
        markdown_path: Some(record.markdown_path),
        markdown,
        saved_at_unix_ms: Some(record.saved_at_unix_ms),
        markdown_identity: Some(record.markdown_identity),
        original_identity: record.original_identity,
    })
}

fn ensure_trusted_directory(vault: &VaultLease, path: &Path) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Knowledge storage path is not a trusted directory".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => vault
            .directory
            .create_dir(path)
            .map_err(|error| format!("Knowledge storage directory cannot be created: {error}")),
        Err(error) => Err(format!(
            "Knowledge storage directory cannot be inspected: {error}"
        )),
    }
}

fn remove_uncommitted_markdown(
    vault: &VaultLease,
    markdown_path: &Path,
    record_path: &Path,
) -> Result<(), String> {
    match vault.directory.symlink_metadata(record_path) {
        Ok(_) => return Err("Knowledge revision metadata already exists".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Knowledge revision metadata cannot be inspected: {error}"
            ))
        }
    }
    match vault.directory.symlink_metadata(markdown_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Uncommitted Markdown path is not a regular file".to_owned())
        }
        Ok(_) => vault
            .directory
            .remove_file(markdown_path)
            .map_err(|error| format!("Uncommitted Markdown cannot be recovered: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Uncommitted Markdown cannot be inspected: {error}")),
    }
}

fn starter_markdown(name: &str, original_path: &str, identity: &ContentIdentity) -> String {
    let title = Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| name.into());
    let yaml_title = serde_json::to_string(title.as_ref())
        .expect("a canonical filename stem always serializes as a JSON string");
    format!(
        "---\ntitle: {yaml_title}\nsource_sha256: {}\nstatus: draft\n---\n\n# {title}\n\n> [!SOURCE]\n> Archived original: [[{original_path}]]\n",
        identity.digest
    )
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
    use super::{open_committed_revision, open_document, save_document};
    use crate::archive::{
        commit_plan_with_faults, ArchivePlan, ArchivePlanItem, TransactionFaults,
    };
    use crate::identity::ContentIdentity;
    use crate::naming::schema::{NamingDecisionEvidence, NamingFact, NamingFactKind};
    use crate::vault::VaultAuthorityRegistry;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SOURCE_BYTES: &[u8] = b"knowledge source bytes\n";
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn committed_fixture() -> (PathBuf, PathBuf, crate::vault::VaultLease, String) {
        let root = std::env::temp_dir().join(format!(
            "aiks-knowledge-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create fixture root");
        let root = root.canonicalize().expect("canonical fixture root");
        let source = root.join("source.txt");
        fs::write(&source, SOURCE_BYTES).expect("write source");
        let vault_path = root.join("vault");
        fs::create_dir(&vault_path).expect("create Vault");
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults.authorize_path(&vault_path).expect("authorize Vault");
        let lease = vaults.lease(&summary.authority_id).expect("lease Vault");
        let identity =
            ContentIdentity::from_reader(Cursor::new(SOURCE_BYTES)).expect("hash source");
        let plan = ArchivePlan {
            plan_id: "knowledge-plan".to_owned(),
            plan_version: 2,
            proposal_id: "knowledge-proposal".to_owned(),
            naming_batch_id: "knowledge-naming".to_owned(),
            authority_id: summary.authority_id,
            vault_path: vault_path.to_string_lossy().into_owned(),
            expires_at_unix_ms: u64::MAX,
            confirmation_nonce: "knowledge-confirmation".to_owned(),
            source_preserved: true,
            items: vec![ArchivePlanItem {
                item_id: "knowledge-item".to_owned(),
                source_path: source.to_string_lossy().into_owned(),
                destination_path: format!("Originals/{}/Knowledge-source.txt", identity.digest),
                original_name: "source.txt".to_owned(),
                canonical_name: "Knowledge-source.txt".to_owned(),
                naming: NamingDecisionEvidence {
                    naming_proposal_id: "knowledge-name".to_owned(),
                    original_name: "source.txt".to_owned(),
                    canonical_name: "Knowledge-source.txt".to_owned(),
                    policy_id: "canonical-v1".to_owned(),
                    policy_version: "1.0.0".to_owned(),
                    applied_rule: "ordered-cited-facts-v1".to_owned(),
                    facts: vec![NamingFact {
                        kind: NamingFactKind::Subject,
                        value: "Knowledge source".to_owned(),
                        evidence_location: "page:1".to_owned(),
                    }],
                },
                byte_size: SOURCE_BYTES.len() as u64,
                identity,
            }],
        };
        let result = commit_plan_with_faults(plan, &lease, TransactionFaults::default());
        (root, source, lease, result.items[0].operation_id.clone())
    }

    #[test]
    fn gates_append_only_markdown_revisions_on_a_verified_archive() {
        let (root, source, lease, operation_id) = committed_fixture();
        assert!(open_document(&lease, "missing-operation").is_err());

        let starter = open_document(&lease, &operation_id).expect("open starter");
        assert_eq!(starter.revision, 0);
        assert!(starter.markdown.contains("Knowledge-source"));
        assert!(starter.markdown_path.is_none());

        let first =
            save_document(&lease, &operation_id, 0, "# First\n").expect("save first revision");
        assert_eq!(first.revision, 1);
        assert_eq!(first.markdown, "# First\n");
        let second =
            save_document(&lease, &operation_id, 1, "# Second\n").expect("save second revision");
        assert_eq!(second.revision, 2);
        assert!(save_document(&lease, &operation_id, 1, "# Stale\n").is_err());

        let reopened = open_document(&lease, &operation_id).expect("reopen latest");
        assert_eq!(reopened.revision, 2);
        assert_eq!(reopened.markdown, "# Second\n");
        assert_eq!(fs::read(&source).expect("read source"), SOURCE_BYTES);
        assert_eq!(
            fs::read_dir(root.join("vault/Knowledge").join(&operation_id))
                .expect("read revisions")
                .count(),
            2
        );
        drop(lease);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_oversized_markdown_without_touching_the_archive() {
        let (root, source, lease, operation_id) = committed_fixture();
        let oversized = "x".repeat(1024 * 1024 + 1);
        assert!(save_document(&lease, &operation_id, 0, &oversized).is_err());
        assert_eq!(fs::read(&source).expect("read source"), SOURCE_BYTES);
        assert_eq!(open_document(&lease, &operation_id).unwrap().revision, 0);
        drop(lease);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn recovers_an_uncommitted_markdown_revision_before_retrying() {
        let (root, source, lease, operation_id) = committed_fixture();
        let orphan_directory = root.join("vault/Knowledge").join(&operation_id);
        fs::create_dir(&orphan_directory).expect("create orphan directory");
        fs::write(orphan_directory.join("00000001.md"), "# Orphan\n")
            .expect("write orphan revision");

        let saved = save_document(&lease, &operation_id, 0, "# Recovered\n")
            .expect("replace uncommitted revision");

        assert_eq!(saved.revision, 1);
        assert_eq!(
            open_document(&lease, &operation_id).unwrap().markdown,
            "# Recovered\n"
        );
        assert_eq!(fs::read(source).expect("read source"), SOURCE_BYTES);
        drop(lease);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn opens_one_exact_committed_revision_with_current_provenance() {
        let (root, source, lease, operation_id) = committed_fixture();
        save_document(&lease, &operation_id, 0, "# First\n\nEvidence one.\n")
            .expect("save first revision");
        save_document(&lease, &operation_id, 1, "# Second\n\nEvidence two.\n")
            .expect("save second revision");

        let first =
            open_committed_revision(&lease, &operation_id, 1).expect("open exact first revision");
        assert_eq!(first.revision, 1);
        assert_eq!(first.markdown, "# First\n\nEvidence one.\n");
        assert!(open_committed_revision(&lease, &operation_id, 0).is_err());
        assert!(open_committed_revision(&lease, &operation_id, 3).is_err());

        let first_path = root
            .join("vault/Knowledge")
            .join(&operation_id)
            .join("00000001.md");
        fs::write(first_path, "# Tampered\n").expect("tamper revision");
        assert!(open_committed_revision(&lease, &operation_id, 1).is_err());
        assert_eq!(fs::read(source).expect("read source"), SOURCE_BYTES);
        drop(lease);
        fs::remove_dir_all(root).expect("remove fixture");
    }
}

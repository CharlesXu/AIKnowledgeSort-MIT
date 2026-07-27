use super::plan::{ArchivePlan, ArchivePlanItem};
use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use crate::identity::ContentIdentity;
use crate::vault::records::{read_json, validate_relative_path, write_new_json};
use crate::vault::VaultLease;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Cursor, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_OPERATION_RECORDS: usize = 10_000;
const OPERATION_SCHEMA_VERSION: u32 = 1;
const REGISTRATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveCommitStatus {
    Committed,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveItemStatus {
    Committed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveItemResult {
    pub operation_id: String,
    pub item_id: String,
    pub destination_path: String,
    pub identity: ContentIdentity,
    pub status: ArchiveItemStatus,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveCommitResult {
    pub plan_id: String,
    pub status: ArchiveCommitStatus,
    pub items: Vec<ArchiveItemResult>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TransactionFaults {
    pub corrupt_staging: bool,
    pub reject_staging_read: bool,
    pub reject_registration: bool,
    pub stop_after_copy: bool,
    pub stop_after_exposure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
enum OperationState {
    Proposed,
    Copying,
    Verified,
    Committed,
    Failed,
    Abandoned,
}

impl OperationState {
    fn file_label(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Copying => "copying",
            Self::Verified => "verified",
            Self::Committed => "committed",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationRecord {
    schema_version: u32,
    operation_id: String,
    sequence: u32,
    state: OperationState,
    actor: String,
    recorded_at_unix_ms: u64,
    plan_id: String,
    confirmation_binding_sha256: String,
    authority_id: String,
    item_id: String,
    source_path: String,
    destination_path: String,
    byte_size: u64,
    identity: ContentIdentity,
    invariant_result: String,
    outcome: String,
    failure_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct OriginalRegistration {
    schema_version: u32,
    operation_id: String,
    authority_id: String,
    relative_path: String,
    source_path: String,
    original_format: String,
    byte_size: u64,
    identity: ContentIdentity,
}

struct OperationContext {
    record: OperationRecord,
    staging_path: PathBuf,
    pending_registration_path: PathBuf,
    registration_path: PathBuf,
    destination_path: PathBuf,
    target_exposed: bool,
}

enum TransactionFailure {
    Rejected(String),
    Interrupted(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ReconciliationReport {
    pub recovered: usize,
    pub abandoned: usize,
}

pub(crate) fn commit_plan_with_faults(
    plan: ArchivePlan,
    vault: &VaultLease,
    faults: TransactionFaults,
) -> ArchiveCommitResult {
    let plan_id = plan.plan_id.clone();
    let mut results = Vec::with_capacity(plan.items.len());
    if plan.authority_id != vault.summary.authority_id {
        for item in plan.items {
            results.push(failed_without_operation(
                item,
                "Archive plan Vault authority is no longer current",
            ));
        }
        return ArchiveCommitResult {
            plan_id,
            status: ArchiveCommitStatus::Failed,
            items: results,
        };
    }

    for item in plan.items.clone() {
        results.push(commit_one(&plan, item, vault, faults));
    }
    let committed = results
        .iter()
        .filter(|result| result.status == ArchiveItemStatus::Committed)
        .count();
    let status = match committed {
        0 => ArchiveCommitStatus::Failed,
        count if count == results.len() => ArchiveCommitStatus::Committed,
        _ => ArchiveCommitStatus::Partial,
    };
    ArchiveCommitResult {
        plan_id,
        status,
        items: results,
    }
}

fn failed_without_operation(item: ArchivePlanItem, reason: &str) -> ArchiveItemResult {
    ArchiveItemResult {
        operation_id: String::new(),
        item_id: item.item_id,
        destination_path: item.destination_path,
        identity: item.identity,
        status: ArchiveItemStatus::Failed,
        failure_reason: Some(reason.to_owned()),
    }
}

fn commit_one(
    plan: &ArchivePlan,
    item: ArchivePlanItem,
    vault: &VaultLease,
    faults: TransactionFaults,
) -> ArchiveItemResult {
    let operation_id = Uuid::new_v4().simple().to_string();
    let mut context = OperationContext::new(plan, &item, operation_id.clone());
    if let Err(reason) = persist_operation(vault, &context.record) {
        return ArchiveItemResult {
            operation_id,
            item_id: item.item_id,
            destination_path: item.destination_path,
            identity: item.identity,
            status: ArchiveItemStatus::Failed,
            failure_reason: Some(reason),
        };
    }

    match execute_operation(&mut context, vault, faults) {
        Ok(()) => ArchiveItemResult {
            operation_id,
            item_id: item.item_id,
            destination_path: item.destination_path,
            identity: item.identity,
            status: ArchiveItemStatus::Committed,
            failure_reason: None,
        },
        Err(TransactionFailure::Interrupted(reason)) => ArchiveItemResult {
            operation_id,
            item_id: item.item_id,
            destination_path: item.destination_path,
            identity: item.identity,
            status: ArchiveItemStatus::Failed,
            failure_reason: Some(reason),
        },
        Err(TransactionFailure::Rejected(reason)) => {
            cleanup_rejected_operation(vault, &context);
            transition(
                vault,
                &mut context.record,
                OperationState::Failed,
                "rejected",
                "not-committed",
                Some(&reason),
            );
            ArchiveItemResult {
                operation_id,
                item_id: item.item_id,
                destination_path: item.destination_path,
                identity: item.identity,
                status: ArchiveItemStatus::Failed,
                failure_reason: Some(reason),
            }
        }
    }
}

impl OperationContext {
    fn new(plan: &ArchivePlan, item: &ArchivePlanItem, operation_id: String) -> Self {
        let confirmation_binding =
            ContentIdentity::from_reader(Cursor::new(plan.confirmation_nonce.as_bytes()))
                .expect("in-memory confirmation hashing cannot fail");
        let destination_path = PathBuf::from(&item.destination_path);
        Self {
            staging_path: Path::new(".aiks/staging").join(format!("{operation_id}.part")),
            pending_registration_path: Path::new(".aiks/pending-registrations")
                .join(format!("{operation_id}.json")),
            registration_path: Path::new(".aiks/registrations")
                .join(format!("{operation_id}.json")),
            destination_path,
            target_exposed: false,
            record: OperationRecord {
                schema_version: OPERATION_SCHEMA_VERSION,
                operation_id,
                sequence: 0,
                state: OperationState::Proposed,
                actor: "desktop-user".to_owned(),
                recorded_at_unix_ms: unix_time_ms(),
                plan_id: plan.plan_id.clone(),
                confirmation_binding_sha256: confirmation_binding.digest,
                authority_id: plan.authority_id.clone(),
                item_id: item.item_id.clone(),
                source_path: item.source_path.clone(),
                destination_path: item.destination_path.clone(),
                byte_size: item.byte_size,
                identity: item.identity.clone(),
                invariant_result: "pending".to_owned(),
                outcome: "proposed".to_owned(),
                failure_reason: None,
            },
        }
    }
}

fn execute_operation(
    context: &mut OperationContext,
    vault: &VaultLease,
    faults: TransactionFaults,
) -> Result<(), TransactionFailure> {
    validate_destination(&context.destination_path, &context.record.identity)
        .map_err(TransactionFailure::Rejected)?;
    let mut source = open_verified_source(&context.record).map_err(TransactionFailure::Rejected)?;
    transition_required(
        vault,
        &mut context.record,
        OperationState::Copying,
        "pending",
        "copying",
        None,
    )?;

    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut staging = vault
        .directory
        .open_with(&context.staging_path, &options)
        .map_err(|error| {
            TransactionFailure::Rejected(format!("Archive staging file cannot be created: {error}"))
        })?;
    io::copy(&mut source, &mut staging)
        .map_err(|error| TransactionFailure::Rejected(format!("Archive copy failed: {error}")))?;
    staging.flush().map_err(|error| {
        TransactionFailure::Rejected(format!("Archive staging flush failed: {error}"))
    })?;
    staging.sync_all().map_err(|error| {
        TransactionFailure::Rejected(format!("Archive staging sync failed: {error}"))
    })?;

    if faults.stop_after_copy {
        return Err(TransactionFailure::Interrupted(
            "Simulated interruption after archive copy".to_owned(),
        ));
    }
    if faults.corrupt_staging {
        staging.seek(SeekFrom::Start(0)).map_err(|error| {
            TransactionFailure::Rejected(format!("Fault injection seek failed: {error}"))
        })?;
        staging.write_all(b"!").map_err(|error| {
            TransactionFailure::Rejected(format!("Fault injection write failed: {error}"))
        })?;
        staging.sync_all().map_err(|error| {
            TransactionFailure::Rejected(format!("Fault injection sync failed: {error}"))
        })?;
    }
    drop(staging);
    if faults.reject_staging_read {
        return Err(TransactionFailure::Rejected(
            "Archive staging output is unreadable".to_owned(),
        ));
    }
    let staged_identity =
        hash_vault_file(vault, &context.staging_path).map_err(TransactionFailure::Rejected)?;
    if staged_identity != context.record.identity {
        return Err(TransactionFailure::Rejected(
            "Archive staging SHA-256 does not match the reviewed source".to_owned(),
        ));
    }
    open_verified_source(&context.record).map_err(TransactionFailure::Rejected)?;
    transition_required(
        vault,
        &mut context.record,
        OperationState::Verified,
        "sha256-match",
        "verified",
        None,
    )?;

    if faults.reject_registration {
        return Err(TransactionFailure::Rejected(
            "Archive registration was rejected".to_owned(),
        ));
    }
    let registration = registration_from_record(&context.record);
    write_new_json(
        &vault.directory,
        &context.pending_registration_path,
        &registration,
    )
    .map_err(TransactionFailure::Rejected)?;
    ensure_destination_parent(vault, &context.destination_path)
        .map_err(TransactionFailure::Rejected)?;
    require_absent(
        &vault.directory,
        &context.destination_path,
        "archive destination",
    )
    .map_err(TransactionFailure::Rejected)?;
    vault
        .directory
        .hard_link(
            &context.staging_path,
            &vault.directory,
            &context.destination_path,
        )
        .map_err(|error| {
            TransactionFailure::Rejected(format!(
                "Archive destination cannot be exposed atomically: {error}"
            ))
        })?;
    context.target_exposed = true;
    if faults.stop_after_exposure {
        return Err(TransactionFailure::Interrupted(
            "Simulated interruption after archive exposure".to_owned(),
        ));
    }
    promote_registration(vault, context).map_err(TransactionFailure::Rejected)?;
    transition_required(
        vault,
        &mut context.record,
        OperationState::Committed,
        "registered-original-preserved",
        "committed",
        None,
    )?;
    let _ = remove_file_if_present(&vault.directory, &context.staging_path);
    Ok(())
}

fn open_verified_source(record: &OperationRecord) -> Result<cap_std::fs::File, String> {
    let root = open_trusted_drop_root(PathBuf::from(&record.source_path));
    let mut file = match root {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Reviewed archive source is no longer a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { message, .. } => return Err(message),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("Reviewed source metadata is unreadable: {error}"))?;
    if metadata.len() != record.byte_size {
        return Err("Reviewed source byte size changed after plan review".to_owned());
    }
    let identity = ContentIdentity::from_reader(&mut file)
        .map_err(|error| format!("Reviewed source cannot be hashed: {error}"))?;
    if identity != record.identity {
        return Err("Reviewed source SHA-256 changed after plan review".to_owned());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("Reviewed source cannot be rewound: {error}"))?;
    Ok(file)
}

fn hash_vault_file(vault: &VaultLease, relative: &Path) -> Result<ContentIdentity, String> {
    validate_relative_path(relative)?;
    let metadata = vault
        .directory
        .symlink_metadata(relative)
        .map_err(|error| format!("Vault file metadata is unreadable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Vault file is not a regular no-link file".to_owned());
    }
    let mut file = vault
        .directory
        .open(relative)
        .map_err(|error| format!("Vault file is unreadable: {error}"))?;
    ContentIdentity::from_reader(&mut file)
        .map_err(|error| format!("Vault file cannot be hashed: {error}"))
}

fn validate_destination(path: &Path, identity: &ContentIdentity) -> Result<(), String> {
    validate_relative_path(path)?;
    let components = path.components().collect::<Vec<_>>();
    if components.len() != 3
        || components[0] != Component::Normal("Originals".as_ref())
        || components[1] != Component::Normal(identity.digest.as_ref())
    {
        return Err("Archive destination is outside the reviewed intake layout".to_owned());
    }
    Ok(())
}

fn ensure_destination_parent(vault: &VaultLease, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "Archive destination parent is missing".to_owned())?;
    match vault.directory.symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Archive destination parent is not a regular directory".to_owned())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => vault
            .directory
            .create_dir(parent)
            .map_err(|error| format!("Archive destination parent cannot be created: {error}")),
        Err(error) => Err(format!(
            "Archive destination parent cannot be inspected: {error}"
        )),
    }
}

fn require_absent(directory: &cap_std::fs::Dir, path: &Path, label: &str) -> Result<(), String> {
    match directory.symlink_metadata(path) {
        Ok(_) => Err(format!("{label} already exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{label} cannot be inspected: {error}")),
    }
}

fn promote_registration(vault: &VaultLease, context: &OperationContext) -> Result<(), String> {
    require_absent(
        &vault.directory,
        &context.registration_path,
        "archive registration",
    )?;
    vault
        .directory
        .rename(
            &context.pending_registration_path,
            &vault.directory,
            &context.registration_path,
        )
        .map_err(|error| format!("Archive registration cannot be committed: {error}"))
}

fn persist_operation(vault: &VaultLease, record: &OperationRecord) -> Result<(), String> {
    let path = operation_record_path(record);
    write_new_json(&vault.directory, &path, record)
}

fn operation_record_path(record: &OperationRecord) -> PathBuf {
    Path::new(".aiks/operations").join(format!(
        "{}-{:04}-{}.json",
        record.operation_id,
        record.sequence,
        record.state.file_label()
    ))
}

fn transition_required(
    vault: &VaultLease,
    record: &mut OperationRecord,
    state: OperationState,
    invariant_result: &str,
    outcome: &str,
    failure_reason: Option<&str>,
) -> Result<(), TransactionFailure> {
    record.sequence = record.sequence.saturating_add(1);
    record.state = state;
    record.recorded_at_unix_ms = unix_time_ms();
    record.invariant_result = invariant_result.to_owned();
    record.outcome = outcome.to_owned();
    record.failure_reason = failure_reason.map(bounded_failure);
    persist_operation(vault, record).map_err(TransactionFailure::Rejected)
}

fn transition(
    vault: &VaultLease,
    record: &mut OperationRecord,
    state: OperationState,
    invariant_result: &str,
    outcome: &str,
    failure_reason: Option<&str>,
) {
    let _ = transition_required(
        vault,
        record,
        state,
        invariant_result,
        outcome,
        failure_reason,
    );
}

fn registration_from_record(record: &OperationRecord) -> OriginalRegistration {
    let original_format = Path::new(&record.source_path)
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_default();
    OriginalRegistration {
        schema_version: REGISTRATION_SCHEMA_VERSION,
        operation_id: record.operation_id.clone(),
        authority_id: record.authority_id.clone(),
        relative_path: record.destination_path.clone(),
        source_path: record.source_path.clone(),
        original_format,
        byte_size: record.byte_size,
        identity: record.identity.clone(),
    }
}

fn cleanup_rejected_operation(vault: &VaultLease, context: &OperationContext) {
    let _ = remove_file_if_present(&vault.directory, &context.staging_path);
    let _ = remove_file_if_present(&vault.directory, &context.pending_registration_path);
    let _ = remove_file_if_present(&vault.directory, &context.registration_path);
    if context.target_exposed {
        let _ = remove_file_if_present(&vault.directory, &context.destination_path);
    }
}

fn remove_file_if_present(directory: &cap_std::fs::Dir, path: &Path) -> Result<(), String> {
    match directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "Operation-owned cleanup path is not a regular file: {}",
            path.display()
        )),
        Ok(_) => directory
            .remove_file(path)
            .map_err(|error| format!("Operation-owned file cannot be removed: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Operation-owned cleanup path cannot be inspected: {error}"
        )),
    }
}

fn bounded_failure(value: &str) -> String {
    value.chars().take(512).collect()
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub(crate) fn reconcile_vault(vault: &VaultLease) -> Result<ReconciliationReport, String> {
    let latest = latest_operation_records(vault)?;
    let mut report = ReconciliationReport::default();
    for mut record in latest.into_values() {
        record.identity.validate()?;
        validate_destination(Path::new(&record.destination_path), &record.identity)?;
        let context = context_from_record(record.clone());
        match record.state {
            OperationState::Committed => {
                verify_committed(vault, &record)?;
                remove_file_if_present(&vault.directory, &context.staging_path)?;
                remove_file_if_present(&vault.directory, &context.pending_registration_path)?;
            }
            OperationState::Failed | OperationState::Abandoned => {
                remove_file_if_present(&vault.directory, &context.staging_path)?;
                remove_file_if_present(&vault.directory, &context.pending_registration_path)?;
            }
            OperationState::Proposed | OperationState::Copying => {
                remove_file_if_present(&vault.directory, &context.staging_path)?;
                remove_file_if_present(&vault.directory, &context.pending_registration_path)?;
                remove_file_if_present(&vault.directory, &context.registration_path)?;
                transition_required(
                    vault,
                    &mut record,
                    OperationState::Abandoned,
                    "source-preserved",
                    "abandoned",
                    Some("Interrupted archive operation was safely abandoned"),
                )
                .map_err(transaction_failure_text)?;
                report.abandoned += 1;
            }
            OperationState::Verified => {
                if vault
                    .directory
                    .symlink_metadata(&context.destination_path)
                    .is_ok()
                {
                    let identity = hash_vault_file(vault, &context.destination_path)?;
                    if identity != record.identity {
                        return Err(
                            "Exposed archive target does not match its operation identity"
                                .to_owned(),
                        );
                    }
                    recover_registration(vault, &record, &context)?;
                    transition_required(
                        vault,
                        &mut record,
                        OperationState::Committed,
                        "registered-original-preserved",
                        "committed",
                        None,
                    )
                    .map_err(transaction_failure_text)?;
                    remove_file_if_present(&vault.directory, &context.staging_path)?;
                    report.recovered += 1;
                } else {
                    remove_file_if_present(&vault.directory, &context.staging_path)?;
                    remove_file_if_present(&vault.directory, &context.pending_registration_path)?;
                    remove_file_if_present(&vault.directory, &context.registration_path)?;
                    transition_required(
                        vault,
                        &mut record,
                        OperationState::Abandoned,
                        "source-preserved",
                        "abandoned",
                        Some("Verified archive was not exposed and was safely abandoned"),
                    )
                    .map_err(transaction_failure_text)?;
                    report.abandoned += 1;
                }
            }
        }
    }
    Ok(report)
}

fn context_from_record(record: OperationRecord) -> OperationContext {
    let operation_id = &record.operation_id;
    OperationContext {
        staging_path: Path::new(".aiks/staging").join(format!("{operation_id}.part")),
        pending_registration_path: Path::new(".aiks/pending-registrations")
            .join(format!("{operation_id}.json")),
        registration_path: Path::new(".aiks/registrations").join(format!("{operation_id}.json")),
        destination_path: PathBuf::from(&record.destination_path),
        target_exposed: false,
        record,
    }
}

fn latest_operation_records(
    vault: &VaultLease,
) -> Result<HashMap<String, OperationRecord>, String> {
    let entries = vault
        .directory
        .read_dir(".aiks/operations")
        .map_err(|error| format!("Archive operation journal is unreadable: {error}"))?;
    let mut latest = HashMap::<String, OperationRecord>::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_OPERATION_RECORDS {
            return Err("Archive operation journal exceeds the reconciliation limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Archive operation entry is unreadable: {error}"))?;
        if entry
            .file_type()
            .map_err(|error| format!("Archive operation entry type is unreadable: {error}"))?
            .is_symlink()
        {
            return Err("Archive operation journal contains a link".to_owned());
        }
        let name = entry.file_name();
        if Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
        {
            continue;
        }
        let relative = Path::new(".aiks/operations").join(name);
        let record: OperationRecord = read_json(&vault.directory, &relative)?;
        if record.schema_version != OPERATION_SCHEMA_VERSION {
            return Err("Archive operation record schema is unsupported".to_owned());
        }
        match latest.get(&record.operation_id) {
            Some(existing) if existing.sequence >= record.sequence => {}
            _ => {
                latest.insert(record.operation_id.clone(), record);
            }
        }
    }
    Ok(latest)
}

fn recover_registration(
    vault: &VaultLease,
    record: &OperationRecord,
    context: &OperationContext,
) -> Result<(), String> {
    match vault.directory.symlink_metadata(&context.registration_path) {
        Ok(_) => {
            let registration: OriginalRegistration =
                read_json(&vault.directory, &context.registration_path)?;
            if registration != registration_from_record(record) {
                return Err(
                    "Committed archive registration does not match its operation".to_owned(),
                );
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match vault
                .directory
                .symlink_metadata(&context.pending_registration_path)
            {
                Ok(_) => promote_registration(vault, context),
                Err(error) if error.kind() == io::ErrorKind::NotFound => write_new_json(
                    &vault.directory,
                    &context.registration_path,
                    &registration_from_record(record),
                ),
                Err(error) => Err(format!(
                    "Pending archive registration cannot be inspected: {error}"
                )),
            }
        }
        Err(error) => Err(format!(
            "Archive registration cannot be inspected during recovery: {error}"
        )),
    }
}

fn verify_committed(vault: &VaultLease, record: &OperationRecord) -> Result<(), String> {
    let target_identity = hash_vault_file(vault, Path::new(&record.destination_path))?;
    if target_identity != record.identity {
        return Err("Committed archive target failed SHA-256 verification".to_owned());
    }
    let registration_path =
        Path::new(".aiks/registrations").join(format!("{}.json", record.operation_id));
    let registration: OriginalRegistration = read_json(&vault.directory, &registration_path)?;
    if registration != registration_from_record(record) {
        return Err("Committed archive registration failed verification".to_owned());
    }
    Ok(())
}

fn transaction_failure_text(failure: TransactionFailure) -> String {
    match failure {
        TransactionFailure::Rejected(reason) | TransactionFailure::Interrupted(reason) => reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{commit_plan_with_faults, reconcile_vault, ArchiveItemStatus, TransactionFaults};
    use crate::archive::plan::{ArchivePlan, ArchivePlanItem};
    use crate::identity::ContentIdentity;
    use crate::vault::VaultAuthorityRegistry;
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    const SOURCE_BYTES: &[u8] = b"verified original bytes\n";

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new() -> Self {
            let unique = format!(
                "aiknowledgesort-archive-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir(&root).expect("create generated archive tree");
            Self {
                root: root.canonicalize().expect("canonical archive tree"),
            }
        }

        fn source(&self) -> PathBuf {
            let path = self.root.join("source.txt");
            fs::write(&path, SOURCE_BYTES).expect("write generated source");
            path
        }

        fn vault(&self) -> PathBuf {
            let path = self.root.join("vault");
            fs::create_dir(&path).expect("create generated Vault");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove generated archive tree");
        }
    }

    fn identity() -> ContentIdentity {
        ContentIdentity::from_reader(Cursor::new(SOURCE_BYTES)).expect("hash generated source")
    }

    fn plan(source: &Path, authority_id: &str) -> ArchivePlan {
        let identity = identity();
        ArchivePlan {
            plan_id: "reviewed-plan".to_owned(),
            plan_version: 1,
            proposal_id: "reviewed-proposal".to_owned(),
            authority_id: authority_id.to_owned(),
            vault_path: "trusted-vault".to_owned(),
            expires_at_unix_ms: u64::MAX,
            confirmation_nonce: "single-use-confirmation".to_owned(),
            source_preserved: true,
            items: vec![ArchivePlanItem {
                item_id: "reviewed-item".to_owned(),
                source_path: source.to_string_lossy().into_owned(),
                destination_path: format!("Originals/{}/source.txt", identity.digest),
                byte_size: SOURCE_BYTES.len() as u64,
                identity,
            }],
        }
    }

    fn directory_is_empty(path: &Path) -> bool {
        path.read_dir()
            .expect("read generated directory")
            .next()
            .is_none()
    }

    #[test]
    fn commits_one_verified_registration_and_preserves_the_source() {
        let tree = TempTree::new();
        let source = tree.source();
        let vault_path = tree.vault();
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults
            .authorize_path(&vault_path)
            .expect("authorize generated Vault");
        let lease = vaults
            .lease(&summary.authority_id)
            .expect("lease generated Vault");
        let plan = plan(&source, &summary.authority_id);
        let destination = vault_path.join(&plan.items[0].destination_path);

        let result = commit_plan_with_faults(plan, &lease, TransactionFaults::default());

        assert_eq!(result.items[0].status, ArchiveItemStatus::Committed);
        assert_eq!(fs::read(&source).expect("read source"), SOURCE_BYTES);
        assert_eq!(
            fs::read(destination).expect("read destination"),
            SOURCE_BYTES
        );
        assert!(!directory_is_empty(&vault_path.join(".aiks/registrations")));
    }

    #[test]
    fn copy_verification_and_registration_failures_never_expose_a_commit() {
        for faults in [
            TransactionFaults {
                corrupt_staging: true,
                ..TransactionFaults::default()
            },
            TransactionFaults {
                reject_staging_read: true,
                ..TransactionFaults::default()
            },
            TransactionFaults {
                reject_registration: true,
                ..TransactionFaults::default()
            },
        ] {
            let tree = TempTree::new();
            let source = tree.source();
            let vault_path = tree.vault();
            let vaults = VaultAuthorityRegistry::default();
            let summary = vaults
                .authorize_path(&vault_path)
                .expect("authorize generated Vault");
            let lease = vaults
                .lease(&summary.authority_id)
                .expect("lease generated Vault");
            let plan = plan(&source, &summary.authority_id);
            let destination = vault_path.join(&plan.items[0].destination_path);

            let result = commit_plan_with_faults(plan, &lease, faults);

            assert_eq!(result.items[0].status, ArchiveItemStatus::Failed);
            assert_eq!(fs::read(&source).expect("read source"), SOURCE_BYTES);
            assert!(!destination.exists());
            assert!(directory_is_empty(&vault_path.join(".aiks/registrations")));
        }
    }

    #[test]
    fn reconciles_interrupted_copy_to_an_abandoned_non_commit() {
        let tree = TempTree::new();
        let source = tree.source();
        let vault_path = tree.vault();
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults
            .authorize_path(&vault_path)
            .expect("authorize generated Vault");
        let lease = vaults
            .lease(&summary.authority_id)
            .expect("lease generated Vault");
        let plan = plan(&source, &summary.authority_id);
        let destination = vault_path.join(&plan.items[0].destination_path);

        let result = commit_plan_with_faults(
            plan,
            &lease,
            TransactionFaults {
                stop_after_copy: true,
                ..TransactionFaults::default()
            },
        );
        assert_eq!(result.items[0].status, ArchiveItemStatus::Failed);
        assert!(!directory_is_empty(&vault_path.join(".aiks/staging")));

        let report = reconcile_vault(&lease).expect("reconcile interrupted copy");

        assert_eq!(report.abandoned, 1);
        assert!(directory_is_empty(&vault_path.join(".aiks/staging")));
        assert!(!destination.exists());
        assert_eq!(fs::read(source).expect("read source"), SOURCE_BYTES);
    }

    #[test]
    fn reconciles_exposure_before_registration_to_a_verified_commit() {
        let tree = TempTree::new();
        let source = tree.source();
        let vault_path = tree.vault();
        let vaults = VaultAuthorityRegistry::default();
        let summary = vaults
            .authorize_path(&vault_path)
            .expect("authorize generated Vault");
        let lease = vaults
            .lease(&summary.authority_id)
            .expect("lease generated Vault");
        let plan = plan(&source, &summary.authority_id);
        let destination = vault_path.join(&plan.items[0].destination_path);

        let result = commit_plan_with_faults(
            plan,
            &lease,
            TransactionFaults {
                stop_after_exposure: true,
                ..TransactionFaults::default()
            },
        );
        assert_eq!(result.items[0].status, ArchiveItemStatus::Failed);
        assert_eq!(
            fs::read(&destination).expect("read exposed target"),
            SOURCE_BYTES
        );

        let report = reconcile_vault(&lease).expect("reconcile exposed target");

        assert_eq!(report.recovered, 1);
        assert_eq!(
            fs::read(destination).expect("read committed target"),
            SOURCE_BYTES
        );
        assert!(!directory_is_empty(&vault_path.join(".aiks/registrations")));
        assert_eq!(fs::read(source).expect("read source"), SOURCE_BYTES);
    }
}

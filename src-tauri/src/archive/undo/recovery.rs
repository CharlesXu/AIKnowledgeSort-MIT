use super::{
    active_registration_path, confirmation_binding, deactivate_registration, persist_state,
    quarantine_path, read_json, remove_file_if_present, remove_quarantine_directory,
    rollback_archive, rollback_quarantine, staging_path, undone_registration_path,
    verified_external_identity, verify_registration, verify_vault_identity, ArchiveUndoPlan,
    UndoAudit, UndoLifecycleState,
};
use crate::vault::VaultLease;
use std::io;
use std::path::Path;

const MAX_UNDO_OPERATIONS: usize = 10_000;
const MAX_RECORDS_PER_UNDO: usize = 8;

fn read_operation(vault: &VaultLease, undo_id: &str) -> Result<Vec<UndoAudit>, String> {
    validate_id(undo_id)?;
    let directory = Path::new(".aiks/archive-undo").join(undo_id);
    let entries = vault
        .directory
        .read_dir(&directory)
        .map_err(|error| format!("Archive undo audit cannot be read: {error}"))?;
    let mut records = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_RECORDS_PER_UNDO {
            return Err("Archive undo audit exceeds its record limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Archive undo audit entry cannot be read: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Archive undo audit entry type cannot be read: {error}"))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err("Archive undo audit contains an untrusted entry".to_owned());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Archive undo audit filename is not valid UTF-8".to_owned())?;
        let sequence = parse_sequence(&name)?;
        let record: UndoAudit = read_json(&vault.directory, &directory.join(name))?;
        if record.schema_version != 1
            || record.sequence != sequence
            || record.plan.undo_id != undo_id
            || record.plan.authority_id != vault.summary.authority_id
            || confirmation_binding(&record.plan)? != record.plan.confirmation_binding_sha256
        {
            return Err("Archive undo audit binding is invalid".to_owned());
        }
        records.push(record);
    }
    records.sort_by_key(|record| record.sequence);
    validate_history(&records)?;
    Ok(records)
}

fn validate_history(records: &[UndoAudit]) -> Result<(), String> {
    for (index, record) in records.iter().enumerate() {
        if record.sequence as usize != index {
            return Err("Archive undo audit is truncated or reordered".to_owned());
        }
        if index > 0 && record.plan != records[0].plan {
            return Err("Archive undo plan changed after review".to_owned());
        }
    }
    let states = records
        .iter()
        .map(|record| record.state)
        .collect::<Vec<_>>();
    if !matches!(
        states.as_slice(),
        [UndoLifecycleState::Proposed]
            | [UndoLifecycleState::Proposed, UndoLifecycleState::Failed]
            | [UndoLifecycleState::Proposed, UndoLifecycleState::Abandoned]
            | [UndoLifecycleState::Proposed, UndoLifecycleState::Executing]
            | [
                UndoLifecycleState::Proposed,
                UndoLifecycleState::Executing,
                UndoLifecycleState::Committed
            ]
            | [
                UndoLifecycleState::Proposed,
                UndoLifecycleState::Executing,
                UndoLifecycleState::Failed
            ]
            | [
                UndoLifecycleState::Proposed,
                UndoLifecycleState::Executing,
                UndoLifecycleState::Abandoned
            ]
    ) {
        return Err("Archive undo audit contains an invalid lifecycle".to_owned());
    }
    Ok(())
}

pub(crate) fn reconcile_vault(vault: &VaultLease) -> Result<(), String> {
    let root = Path::new(".aiks/archive-undo");
    let entries = vault
        .directory
        .read_dir(root)
        .map_err(|error| format!("Archive undo audit root cannot be read: {error}"))?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_UNDO_OPERATIONS {
            return Err("Archive undo audit exceeds the reconciliation limit".to_owned());
        }
        let entry = entry
            .map_err(|error| format!("Archive undo audit root entry cannot be read: {error}"))?;
        let file_type = entry.file_type().map_err(|error| {
            format!("Archive undo audit root entry type cannot be read: {error}")
        })?;
        if file_type.is_symlink() || !file_type.is_dir() {
            return Err("Archive undo audit root contains an untrusted entry".to_owned());
        }
        let undo_id = entry
            .file_name()
            .into_string()
            .map_err(|_| "Archive undo ID is not valid UTF-8".to_owned())?;
        let records = read_operation(vault, &undo_id)?;
        let latest = records
            .last()
            .ok_or_else(|| "Archive undo audit operation is empty".to_owned())?;
        match latest.state {
            UndoLifecycleState::Proposed => {
                persist_state(
                    vault,
                    &latest.plan,
                    latest.sequence + 1,
                    UndoLifecycleState::Abandoned,
                    "not-mutated",
                    "recovered-unconfirmed-plan",
                    None,
                )?;
            }
            UndoLifecycleState::Executing => reconcile_executing(vault, latest)?,
            UndoLifecycleState::Committed
            | UndoLifecycleState::Failed
            | UndoLifecycleState::Abandoned => {
                let _ = remove_quarantine_directory(vault, &latest.plan);
            }
        }
    }
    Ok(())
}

fn reconcile_executing(vault: &VaultLease, latest: &UndoAudit) -> Result<(), String> {
    let plan = &latest.plan;
    verify_vault_identity(vault, &staging_path(plan), &plan.identity)?;
    let quarantine = quarantine_path(plan)?;
    if vault.directory.symlink_metadata(&quarantine).is_ok() {
        rollback_quarantine(vault, plan)?;
        verify_registration(vault, &active_registration_path(plan), plan)?;
        persist_state(
            vault,
            plan,
            latest.sequence + 1,
            UndoLifecycleState::Abandoned,
            "archive-preserved",
            "recovered-before-trash",
            None,
        )?;
        remove_file_if_present(vault, &staging_path(plan))?;
        return Ok(());
    }
    match vault
        .directory
        .symlink_metadata(Path::new(&plan.archived_relative_path))
    {
        Ok(_) => {
            verify_vault_identity(
                vault,
                Path::new(&plan.archived_relative_path),
                &plan.identity,
            )?;
            verify_registration(vault, &active_registration_path(plan), plan)?;
            persist_state(
                vault,
                plan,
                latest.sequence + 1,
                UndoLifecycleState::Abandoned,
                "archive-preserved",
                "recovered-before-trash",
                None,
            )?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match verified_external_identity(Path::new(&plan.source_path)) {
                Ok(identity) if identity == plan.identity => {
                    match vault
                        .directory
                        .symlink_metadata(active_registration_path(plan))
                    {
                        Ok(_) => deactivate_registration(vault, plan)?,
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {
                            verify_registration(vault, &undone_registration_path(plan), plan)?
                        }
                        Err(error) => {
                            return Err(format!(
                                "Archive undo registration cannot be inspected: {error}"
                            ))
                        }
                    }
                    persist_state(
                        vault,
                        plan,
                        latest.sequence + 1,
                        UndoLifecycleState::Committed,
                        "source-original-reverified",
                        "recovered-archive-registration-undone",
                        None,
                    )?;
                }
                _ => {
                    rollback_archive(vault, plan)?;
                    persist_state(
                        vault,
                        plan,
                        latest.sequence + 1,
                        UndoLifecycleState::Failed,
                        "archive-restored",
                        "recovered-unsafe-undo",
                        Some("Source original was unavailable during archive undo recovery"),
                    )?;
                }
            }
        }
        Err(error) => {
            return Err(format!(
                "Archive undo target cannot be inspected during recovery: {error}"
            ))
        }
    }
    remove_file_if_present(vault, &staging_path(plan))
}

pub(in crate::archive) fn operation_is_undone(
    vault: &VaultLease,
    operation_id: &str,
) -> Result<bool, String> {
    let entries = vault
        .directory
        .read_dir(".aiks/archive-undo")
        .map_err(|error| format!("Archive undo audit root cannot be read: {error}"))?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_UNDO_OPERATIONS {
            return Err("Archive undo audit exceeds the verification limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Archive undo audit entry cannot be read: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("Archive undo audit entry type cannot be read: {error}"))?
            .is_dir()
        {
            return Err("Archive undo audit root contains an untrusted entry".to_owned());
        }
        let undo_id = entry
            .file_name()
            .into_string()
            .map_err(|_| "Archive undo ID is not valid UTF-8".to_owned())?;
        let records = read_operation(vault, &undo_id)?;
        let latest = records
            .last()
            .ok_or_else(|| "Archive undo audit operation is empty".to_owned())?;
        if latest.plan.operation_id == operation_id && latest.state == UndoLifecycleState::Committed
        {
            verify_committed_undo(vault, &latest.plan)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn verify_committed_undo(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    match vault
        .directory
        .symlink_metadata(Path::new(&plan.archived_relative_path))
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err("Undone archive destination unexpectedly exists".to_owned()),
        Err(error) => {
            return Err(format!(
                "Undone archive destination cannot be inspected: {error}"
            ))
        }
    }
    match vault
        .directory
        .symlink_metadata(active_registration_path(plan))
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => return Err("Undone archive registration unexpectedly remains active".to_owned()),
        Err(error) => {
            return Err(format!(
                "Undone archive registration cannot be inspected: {error}"
            ))
        }
    }
    verify_registration(vault, &undone_registration_path(plan), plan)
}

fn parse_sequence(name: &str) -> Result<u32, String> {
    let stem = name
        .strip_suffix(".json")
        .ok_or_else(|| "Archive undo audit extension is invalid".to_owned())?;
    if stem.len() != 8 || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Archive undo audit sequence is invalid".to_owned());
    }
    stem.parse()
        .map_err(|_| "Archive undo audit sequence is invalid".to_owned())
}

fn validate_id(value: &str) -> Result<(), String> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Archive undo ID is invalid".to_owned());
    }
    Ok(())
}

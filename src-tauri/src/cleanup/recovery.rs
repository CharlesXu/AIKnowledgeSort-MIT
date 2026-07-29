use super::{
    persist_state, read_json, verified_registered_original, verified_source_identity, CleanupAudit,
    CleanupLifecycleState, CleanupPlan,
};
use crate::vault::VaultLease;
use serde::Serialize;
use std::io;
use std::path::Path;

const MAX_CLEANUP_OPERATIONS: usize = 10_000;
const MAX_RECORDS_PER_OPERATION: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReconciliationReport {
    pub recovered_committed: usize,
    pub abandoned: usize,
    pub failed: usize,
}

pub(crate) fn reconcile_vault(vault: &VaultLease) -> Result<CleanupReconciliationReport, String> {
    let mut report = CleanupReconciliationReport::default();
    let root = Path::new(".aiks/cleanup");
    let entries = vault
        .directory
        .read_dir(root)
        .map_err(|error| format!("Cleanup audit root cannot be read: {error}"))?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_CLEANUP_OPERATIONS {
            return Err("Cleanup audit exceeds the reconciliation limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Cleanup audit entry cannot be read: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Cleanup audit entry type cannot be read: {error}"))?;
        if file_type.is_symlink() || !file_type.is_dir() {
            return Err("Cleanup audit root contains an untrusted entry".to_owned());
        }
        let plan_id = entry
            .file_name()
            .into_string()
            .map_err(|_| "Cleanup audit plan id is not valid UTF-8".to_owned())?;
        let records = read_operation(vault, &plan_id)?;
        let latest = records
            .last()
            .ok_or_else(|| "Cleanup audit operation is empty".to_owned())?;
        match latest.state {
            CleanupLifecycleState::Proposed => {
                persist_state(
                    vault,
                    &latest.plan,
                    latest.sequence + 1,
                    CleanupLifecycleState::Abandoned,
                    "recovered-unconfirmed-plan",
                    None,
                )?;
                report.abandoned += 1;
            }
            CleanupLifecycleState::Executing => {
                reconcile_executing(vault, latest, &mut report)?;
            }
            CleanupLifecycleState::Superseded
            | CleanupLifecycleState::Committed
            | CleanupLifecycleState::Failed
            | CleanupLifecycleState::Abandoned => {}
        }
    }
    Ok(report)
}

fn read_operation(vault: &VaultLease, plan_id: &str) -> Result<Vec<CleanupAudit>, String> {
    validate_plan_id(plan_id)?;
    let directory = Path::new(".aiks/cleanup").join(plan_id);
    let entries = vault
        .directory
        .read_dir(&directory)
        .map_err(|error| format!("Cleanup operation audit cannot be read: {error}"))?;
    let mut records = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= MAX_RECORDS_PER_OPERATION {
            return Err("Cleanup operation audit exceeds its record limit".to_owned());
        }
        let entry =
            entry.map_err(|error| format!("Cleanup operation record cannot be read: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Cleanup operation record type cannot be read: {error}"))?;
        if file_type.is_symlink() || !file_type.is_file() {
            return Err("Cleanup operation contains an untrusted record".to_owned());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Cleanup operation record name is not valid UTF-8".to_owned())?;
        let sequence = parse_sequence(&name)?;
        let relative = directory.join(&name);
        let record: CleanupAudit = read_json(&vault.directory, &relative)?;
        if record.schema_version != 1
            || record.sequence != sequence
            || record.plan.plan_id != plan_id
            || record.plan.authority_id != vault.summary.authority_id
        {
            return Err("Cleanup operation audit binding is invalid".to_owned());
        }
        records.push(record);
    }
    records.sort_by_key(|record| record.sequence);
    validate_history(&records)?;
    Ok(records)
}

fn validate_history(records: &[CleanupAudit]) -> Result<(), String> {
    for (index, record) in records.iter().enumerate() {
        if record.sequence as usize != index {
            return Err("Cleanup operation audit is truncated or reordered".to_owned());
        }
        if index > 0 && record.plan != records[0].plan {
            return Err("Cleanup operation audit plan changed after review".to_owned());
        }
    }
    let states = records
        .iter()
        .map(|record| record.state)
        .collect::<Vec<_>>();
    let valid = matches!(
        states.as_slice(),
        [CleanupLifecycleState::Proposed]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Executing
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Superseded
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Failed
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Abandoned
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Executing,
                CleanupLifecycleState::Committed
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Executing,
                CleanupLifecycleState::Failed
            ]
            | [
                CleanupLifecycleState::Proposed,
                CleanupLifecycleState::Executing,
                CleanupLifecycleState::Abandoned
            ]
    );
    if !valid {
        return Err("Cleanup operation audit contains an invalid lifecycle".to_owned());
    }
    Ok(())
}

fn reconcile_executing(
    vault: &VaultLease,
    latest: &CleanupAudit,
    report: &mut CleanupReconciliationReport,
) -> Result<(), String> {
    if let Err(error) = verify_retained_originals(vault, &latest.plan) {
        persist_recovery_failure(vault, latest, &error)?;
        report.failed += 1;
        return Ok(());
    }
    let mut present = 0usize;
    let mut absent = 0usize;
    for item in &latest.plan.items {
        match std::fs::symlink_metadata(&item.source_path) {
            Ok(_) => match verified_source_identity(Path::new(&item.source_path)) {
                Ok(identity) if identity == item.identity => present += 1,
                Ok(_) => {
                    persist_recovery_failure(
                        vault,
                        latest,
                        "Cleanup source identity changed during recovery",
                    )?;
                    report.failed += 1;
                    return Ok(());
                }
                Err(_) => {
                    persist_recovery_failure(
                        vault,
                        latest,
                        "Cleanup source cannot be verified during recovery",
                    )?;
                    report.failed += 1;
                    return Ok(());
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => absent += 1,
            Err(_) => {
                persist_recovery_failure(
                    vault,
                    latest,
                    "Cleanup source cannot be inspected during recovery",
                )?;
                report.failed += 1;
                return Ok(());
            }
        }
    }

    if absent == latest.plan.items.len() {
        persist_state(
            vault,
            &latest.plan,
            latest.sequence + 1,
            CleanupLifecycleState::Committed,
            "recovered-source-absent-retained-original-verified",
            None,
        )?;
        report.recovered_committed += 1;
    } else if present == latest.plan.items.len() {
        persist_state(
            vault,
            &latest.plan,
            latest.sequence + 1,
            CleanupLifecycleState::Abandoned,
            "recovered-before-source-mutation",
            None,
        )?;
        report.abandoned += 1;
    } else {
        persist_recovery_failure(
            vault,
            latest,
            "Cleanup stopped after a partial source mutation; every retained original was verified",
        )?;
        report.failed += 1;
    }
    Ok(())
}

fn verify_retained_originals(vault: &VaultLease, plan: &CleanupPlan) -> Result<(), String> {
    for item in &plan.items {
        let retained = verified_registered_original(vault, &item.operation_id)?;
        if retained.identity != item.identity
            || retained.source_path != item.source_path
            || Path::new(&vault.summary.display_path)
                .join(retained.relative_path)
                .to_string_lossy()
                != item.retained_path
        {
            return Err("Cleanup retained original changed during recovery".to_owned());
        }
    }
    Ok(())
}

fn persist_recovery_failure(
    vault: &VaultLease,
    latest: &CleanupAudit,
    reason: &str,
) -> Result<(), String> {
    persist_state(
        vault,
        &latest.plan,
        latest.sequence + 1,
        CleanupLifecycleState::Failed,
        "recovered-retained-original-preserved",
        Some(reason),
    )
}

fn parse_sequence(name: &str) -> Result<u32, String> {
    let stem = name
        .strip_suffix(".json")
        .ok_or_else(|| "Cleanup operation record extension is invalid".to_owned())?;
    if stem.len() != 8 || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Cleanup operation record sequence is invalid".to_owned());
    }
    stem.parse()
        .map_err(|_| "Cleanup operation record sequence is invalid".to_owned())
}

fn validate_plan_id(plan_id: &str) -> Result<(), String> {
    if plan_id.len() != 32
        || !plan_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Cleanup audit plan id is invalid".to_owned());
    }
    Ok(())
}

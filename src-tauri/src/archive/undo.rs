mod recovery;

use super::verified_registered_original;
use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use crate::identity::ContentIdentity;
use crate::vault::records::{read_json, write_new_json};
use crate::vault::{VaultAuthorityRegistry, VaultLease};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PLAN_TTL: Duration = Duration::from_secs(5 * 60);

pub(super) use recovery::operation_is_undone;
pub(crate) use recovery::reconcile_vault;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArchiveUndoPlan {
    pub undo_id: String,
    pub plan_version: u32,
    pub operation_id: String,
    pub authority_id: String,
    pub source_path: String,
    pub archived_path: String,
    pub archived_relative_path: String,
    pub byte_size: u64,
    pub identity: ContentIdentity,
    pub expires_at_unix_ms: u64,
    pub confirmation_nonce: String,
    pub confirmation_binding_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveUndoStatus {
    Committed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveUndoResult {
    pub undo_id: String,
    pub operation_id: String,
    pub status: ArchiveUndoStatus,
    pub failure_reason: Option<String>,
}

struct PendingPlan {
    plan: ArchiveUndoPlan,
    expires_at: Instant,
}

#[derive(Clone, Default)]
pub struct ArchiveUndoPlanRegistry {
    plans: Arc<Mutex<HashMap<String, PendingPlan>>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum UndoLifecycleState {
    Proposed,
    Executing,
    Committed,
    Failed,
    Abandoned,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UndoAudit {
    schema_version: u32,
    sequence: u32,
    actor: String,
    recorded_at_unix_ms: u64,
    state: UndoLifecycleState,
    invariant_result: String,
    outcome: String,
    failure_reason: Option<String>,
    plan: ArchiveUndoPlan,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistrationEvidence {
    operation_id: String,
    authority_id: String,
    relative_path: String,
    source_path: String,
    byte_size: u64,
    identity: ContentIdentity,
}

impl ArchiveUndoPlanRegistry {
    fn create_at(
        &self,
        vault: &VaultLease,
        operation_id: &str,
        now: Instant,
        wall_clock: SystemTime,
    ) -> Result<ArchiveUndoPlan, String> {
        if operation_is_undone(vault, operation_id)? {
            return Err("Archive operation was already undone".to_owned());
        }
        let original = verified_registered_original(vault, operation_id)?;
        ensure_no_derived_dependencies(vault, operation_id)?;
        if verified_external_identity(Path::new(&original.source_path))? != original.identity {
            return Err("Archive undo requires a current matching source original".to_owned());
        }
        let archived_path = Path::new(&vault.summary.display_path)
            .join(&original.relative_path)
            .to_string_lossy()
            .into_owned();
        let undo_id = Uuid::new_v4().simple().to_string();
        let confirmation_nonce = Uuid::new_v4().simple().to_string();
        let expires_at_unix_ms = unix_time_ms(wall_clock + PLAN_TTL);
        let mut plan = ArchiveUndoPlan {
            undo_id,
            plan_version: 1,
            operation_id: original.operation_id,
            authority_id: original.authority_id,
            source_path: original.source_path,
            archived_path,
            archived_relative_path: original.relative_path,
            byte_size: original.byte_size,
            identity: original.identity,
            expires_at_unix_ms,
            confirmation_nonce,
            confirmation_binding_sha256: String::new(),
        };
        plan.confirmation_binding_sha256 = confirmation_binding(&plan)?;
        persist_state(
            vault,
            &plan,
            0,
            UndoLifecycleState::Proposed,
            "pending",
            "awaiting-user-confirmation",
            None,
        )?;
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Archive undo plan registry is unavailable".to_owned())?;
        plans.retain(|_, pending| pending.expires_at > now);
        plans.insert(
            plan.undo_id.clone(),
            PendingPlan {
                plan: plan.clone(),
                expires_at: now + PLAN_TTL,
            },
        );
        Ok(plan)
    }

    fn consume_at(
        &self,
        undo_id: &str,
        confirmation_nonce: &str,
        now: Instant,
    ) -> Result<ArchiveUndoPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Archive undo plan registry is unavailable".to_owned())?;
        plans.retain(|_, pending| pending.expires_at > now);
        let pending = plans.get(undo_id).ok_or_else(|| {
            "Archive undo plan is missing, expired, or already consumed".to_owned()
        })?;
        if pending.plan.confirmation_nonce != confirmation_nonce {
            return Err("Archive undo confirmation does not match the reviewed plan".to_owned());
        }
        Ok(plans
            .remove(undo_id)
            .expect("validated archive undo plan remains present")
            .plan)
    }
}

fn confirmation_binding(plan: &ArchiveUndoPlan) -> Result<String, String> {
    let mut bound = plan.clone();
    bound.confirmation_binding_sha256.clear();
    let bytes = serde_json::to_vec(&bound)
        .map_err(|error| format!("Archive undo plan cannot be bound: {error}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn reverify_plan(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    if plan.plan_version != 1
        || plan.authority_id != vault.summary.authority_id
        || confirmation_binding(plan)? != plan.confirmation_binding_sha256
    {
        return Err("Archive undo plan binding is invalid".to_owned());
    }
    ensure_no_derived_dependencies(vault, &plan.operation_id)?;
    let original = verified_registered_original(vault, &plan.operation_id)?;
    let expected_archived_path = Path::new(&vault.summary.display_path)
        .join(&original.relative_path)
        .to_string_lossy()
        .into_owned();
    if original.authority_id != plan.authority_id
        || original.source_path != plan.source_path
        || original.relative_path != plan.archived_relative_path
        || expected_archived_path != plan.archived_path
        || original.byte_size != plan.byte_size
        || original.identity != plan.identity
        || verified_external_identity(Path::new(&plan.source_path))? != plan.identity
    {
        return Err("Archive undo target changed after review".to_owned());
    }
    Ok(())
}

fn ensure_no_derived_dependencies(vault: &VaultLease, operation_id: &str) -> Result<(), String> {
    for path in [
        Path::new(".aiks/knowledge").join(operation_id),
        Path::new("Knowledge").join(operation_id),
    ] {
        match vault.directory.symlink_metadata(&path) {
            Ok(_) => {
                return Err(
                    "Archive undo is unavailable after authoritative knowledge was created"
                        .to_owned(),
                )
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Archive undo dependency state cannot be inspected: {error}"
                ))
            }
        }
    }
    Ok(())
}

fn verified_external_identity(path: &Path) -> Result<ContentIdentity, String> {
    let file = match open_trusted_drop_root(path.to_path_buf()) {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Archive undo source must be a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { .. } => {
            return Err("Archive undo source is unreadable or no longer trusted".to_owned())
        }
    };
    ContentIdentity::from_reader(file)
        .map_err(|error| format!("Archive undo source cannot be hashed: {error}"))
}

fn staging_path(plan: &ArchiveUndoPlan) -> PathBuf {
    Path::new(".aiks/undo-staging").join(format!("{}.part", plan.undo_id))
}

fn active_registration_path(plan: &ArchiveUndoPlan) -> PathBuf {
    Path::new(".aiks/registrations").join(format!("{}.json", plan.operation_id))
}

fn undone_registration_path(plan: &ArchiveUndoPlan) -> PathBuf {
    Path::new(".aiks/undone-registrations").join(format!("{}.json", plan.operation_id))
}

fn quarantine_directory(plan: &ArchiveUndoPlan) -> PathBuf {
    Path::new(".aiks/undo-trash").join(&plan.undo_id)
}

fn quarantine_path(plan: &ArchiveUndoPlan) -> Result<PathBuf, String> {
    let file_name = Path::new(&plan.archived_relative_path)
        .file_name()
        .ok_or_else(|| "Archive undo target filename is missing".to_owned())?;
    Ok(quarantine_directory(plan).join(file_name))
}

fn verify_vault_identity(
    vault: &VaultLease,
    relative_path: &Path,
    expected: &ContentIdentity,
) -> Result<(), String> {
    let metadata = vault
        .directory
        .symlink_metadata(relative_path)
        .map_err(|error| format!("Archive undo Vault file cannot be inspected: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Archive undo Vault file is not a trusted regular file".to_owned());
    }
    let file = vault
        .directory
        .open(relative_path)
        .map_err(|error| format!("Archive undo Vault file cannot be opened: {error}"))?;
    let identity = ContentIdentity::from_reader(file)
        .map_err(|error| format!("Archive undo Vault file cannot be hashed: {error}"))?;
    if &identity != expected {
        return Err("Archive undo Vault file failed SHA-256 verification".to_owned());
    }
    Ok(())
}

fn verify_registration(
    vault: &VaultLease,
    path: &Path,
    plan: &ArchiveUndoPlan,
) -> Result<(), String> {
    let registration: RegistrationEvidence = read_json(&vault.directory, path)?;
    if registration.operation_id != plan.operation_id
        || registration.authority_id != plan.authority_id
        || registration.relative_path != plan.archived_relative_path
        || registration.source_path != plan.source_path
        || registration.byte_size != plan.byte_size
        || registration.identity != plan.identity
    {
        return Err("Archive undo registration evidence changed".to_owned());
    }
    Ok(())
}

fn prepare_staging(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    let staging = staging_path(plan);
    require_absent(vault, &staging, "Archive undo staging file")?;
    vault
        .directory
        .hard_link(
            Path::new(&plan.archived_relative_path),
            &vault.directory,
            &staging,
        )
        .map_err(|error| format!("Archive undo staging link cannot be created: {error}"))?;
    if let Err(error) = verify_vault_identity(vault, &staging, &plan.identity) {
        let _ = remove_file_if_present(vault, &staging);
        return Err(error);
    }
    Ok(())
}

fn quarantine_archive(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<PathBuf, String> {
    let directory = quarantine_directory(plan);
    match vault.directory.symlink_metadata(&directory) {
        Ok(_) => return Err("Archive undo quarantine already exists".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Archive undo quarantine cannot be inspected: {error}"
            ))
        }
    }
    vault
        .directory
        .create_dir(&directory)
        .map_err(|error| format!("Archive undo quarantine cannot be created: {error}"))?;
    let quarantine = quarantine_path(plan)?;
    let result = vault
        .directory
        .rename(
            Path::new(&plan.archived_relative_path),
            &vault.directory,
            &quarantine,
        )
        .map_err(|error| format!("Archived original cannot enter undo quarantine: {error}"))
        .and_then(|_| {
            verify_vault_identity(vault, &quarantine, &plan.identity)?;
            Ok(quarantine.clone())
        });
    if result.is_err() {
        let _ = rollback_quarantine(vault, plan);
    }
    result
}

fn rollback_quarantine(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    let quarantine = quarantine_path(plan)?;
    match vault.directory.symlink_metadata(&quarantine) {
        Ok(_) => {
            require_absent(
                vault,
                Path::new(&plan.archived_relative_path),
                "Archived original restore target",
            )?;
            vault
                .directory
                .rename(
                    &quarantine,
                    &vault.directory,
                    Path::new(&plan.archived_relative_path),
                )
                .map_err(|error| {
                    format!("Archived original cannot leave undo quarantine: {error}")
                })?;
            verify_vault_identity(
                vault,
                Path::new(&plan.archived_relative_path),
                &plan.identity,
            )?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Archive undo quarantine cannot be inspected: {error}"
            ))
        }
    }
    remove_quarantine_directory(vault, plan)
}

fn remove_quarantine_directory(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    let directory = quarantine_directory(plan);
    match vault.directory.symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err("Archive undo quarantine is not a trusted directory".to_owned())
        }
        Ok(_) => vault
            .directory
            .remove_dir(&directory)
            .map_err(|error| format!("Archive undo quarantine cannot be removed: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Archive undo quarantine cannot be inspected: {error}"
        )),
    }
}

trait ArchiveUndoExecutor: Send + Sync {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
}

struct SystemArchiveUndoExecutor;

impl ArchiveUndoExecutor for SystemArchiveUndoExecutor {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path)
            .map_err(|_| "Operating-system trash rejected the archive undo".to_owned())
    }
}

fn execute_with(
    vault: &VaultLease,
    plan: ArchiveUndoPlan,
    executor: &dyn ArchiveUndoExecutor,
) -> ArchiveUndoResult {
    if let Err(error) = reverify_plan(vault, &plan) {
        let _ = persist_state(
            vault,
            &plan,
            1,
            UndoLifecycleState::Failed,
            "not-mutated",
            "rejected",
            Some(&error),
        );
        return failed_result(plan, error);
    }
    if let Err(error) = prepare_staging(vault, &plan).and_then(|_| {
        persist_state(
            vault,
            &plan,
            1,
            UndoLifecycleState::Executing,
            "source-and-archive-verified",
            "moving-archive-to-trash",
            None,
        )
    }) {
        let _ = remove_file_if_present(vault, &staging_path(&plan));
        return failed_result(plan, error);
    }
    let quarantine = match quarantine_archive(vault, &plan) {
        Ok(path) => path,
        Err(error) => {
            let _ = remove_file_if_present(vault, &staging_path(&plan));
            let _ = persist_state(
                vault,
                &plan,
                2,
                UndoLifecycleState::Failed,
                "archive-preserved",
                "quarantine-rejected",
                Some(&error),
            );
            return failed_result(plan, error);
        }
    };
    let quarantine_absolute = Path::new(&vault.summary.display_path).join(&quarantine);
    if let Err(error) = executor.move_to_trash(&quarantine_absolute) {
        let rollback = rollback_quarantine(vault, &plan);
        let _ = remove_file_if_present(vault, &staging_path(&plan));
        let failure = match rollback {
            Ok(()) => error,
            Err(rollback_error) => {
                format!("{error}; archive quarantine rollback failed: {rollback_error}")
            }
        };
        let _ = persist_state(
            vault,
            &plan,
            2,
            UndoLifecycleState::Failed,
            "archive-preserved",
            "trash-rejected",
            Some(&failure),
        );
        return failed_result(plan, failure);
    }
    let _ = remove_quarantine_directory(vault, &plan);
    if let Err(error) = finalize_or_rollback(vault, &plan) {
        let _ = persist_state(
            vault,
            &plan,
            2,
            UndoLifecycleState::Failed,
            "archive-restored",
            "rejected-after-trash",
            Some(&error),
        );
        if verify_vault_identity(
            vault,
            Path::new(&plan.archived_relative_path),
            &plan.identity,
        )
        .is_ok()
            && verify_registration(vault, &active_registration_path(&plan), &plan).is_ok()
        {
            let _ = remove_file_if_present(vault, &staging_path(&plan));
        }
        return failed_result(plan, error);
    }
    if let Err(error) = persist_state(
        vault,
        &plan,
        2,
        UndoLifecycleState::Committed,
        "source-original-reverified",
        "archive-registration-undone",
        None,
    ) {
        return failed_result(
            plan,
            format!("Archive undo completed but outcome audit failed: {error}"),
        );
    }
    let _ = remove_file_if_present(vault, &staging_path(&plan));
    ArchiveUndoResult {
        undo_id: plan.undo_id,
        operation_id: plan.operation_id,
        status: ArchiveUndoStatus::Committed,
        failure_reason: None,
    }
}

fn finalize_or_rollback(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    match verified_external_identity(Path::new(&plan.source_path)) {
        Ok(identity) if identity == plan.identity => {
            deactivate_registration(vault, plan)?;
            match verified_external_identity(Path::new(&plan.source_path)) {
                Ok(identity) if identity == plan.identity => Ok(()),
                _ => {
                    rollback_archive(vault, plan)?;
                    Err(
                        "Archive undo source changed during registration deactivation; archived original was restored"
                            .to_owned(),
                    )
                }
            }
        }
        _ => {
            rollback_archive(vault, plan)?;
            Err(
                "Archive undo source changed during execution; archived original was restored"
                    .to_owned(),
            )
        }
    }
}

fn deactivate_registration(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    let active = active_registration_path(plan);
    let undone = undone_registration_path(plan);
    verify_registration(vault, &active, plan)?;
    require_absent(vault, &undone, "Archive undo registration tombstone")?;
    vault
        .directory
        .rename(&active, &vault.directory, &undone)
        .map_err(|error| format!("Archive registration cannot be deactivated: {error}"))?;
    if let Err(error) = verify_registration(vault, &undone, plan) {
        let _ = vault.directory.rename(&undone, &vault.directory, &active);
        rollback_archive(vault, plan)?;
        return Err(error);
    }
    Ok(())
}

fn rollback_archive(vault: &VaultLease, plan: &ArchiveUndoPlan) -> Result<(), String> {
    let archived = Path::new(&plan.archived_relative_path);
    match vault.directory.symlink_metadata(archived) {
        Ok(_) => verify_vault_identity(vault, archived, &plan.identity)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            verify_vault_identity(vault, &staging_path(plan), &plan.identity)?;
            vault
                .directory
                .hard_link(staging_path(plan), &vault.directory, archived)
                .map_err(|error| format!("Archived original cannot be restored: {error}"))?;
            verify_vault_identity(vault, archived, &plan.identity)?;
        }
        Err(error) => {
            return Err(format!(
                "Archived original restore target cannot be inspected: {error}"
            ))
        }
    }
    let active = active_registration_path(plan);
    let undone = undone_registration_path(plan);
    if vault.directory.symlink_metadata(&active).is_err()
        && vault.directory.symlink_metadata(&undone).is_ok()
    {
        vault
            .directory
            .rename(&undone, &vault.directory, &active)
            .map_err(|error| format!("Archive registration cannot be restored: {error}"))?;
    }
    verify_registration(vault, &active, plan)
}

fn failed_result(plan: ArchiveUndoPlan, error: String) -> ArchiveUndoResult {
    ArchiveUndoResult {
        undo_id: plan.undo_id,
        operation_id: plan.operation_id,
        status: ArchiveUndoStatus::Failed,
        failure_reason: Some(error),
    }
}

fn require_absent(vault: &VaultLease, path: &Path, label: &str) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(_) => Err(format!("{label} already exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{label} cannot be inspected: {error}")),
    }
}

fn remove_file_if_present(vault: &VaultLease, path: &Path) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Archive undo cleanup target is not a regular file".to_owned())
        }
        Ok(_) => vault
            .directory
            .remove_file(path)
            .map_err(|error| format!("Archive undo cleanup target cannot be removed: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Archive undo cleanup target cannot be inspected: {error}"
        )),
    }
}

fn persist_state(
    vault: &VaultLease,
    plan: &ArchiveUndoPlan,
    sequence: u32,
    state: UndoLifecycleState,
    invariant_result: &str,
    outcome: &str,
    failure_reason: Option<&str>,
) -> Result<(), String> {
    let directory = Path::new(".aiks/archive-undo").join(&plan.undo_id);
    match vault.directory.symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("Archive undo audit path is not a trusted directory".to_owned())
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => vault
            .directory
            .create_dir(&directory)
            .map_err(|error| format!("Archive undo audit directory cannot be created: {error}"))?,
        Err(error) => {
            return Err(format!(
                "Archive undo audit directory cannot be inspected: {error}"
            ))
        }
    }
    write_new_json(
        &vault.directory,
        &directory.join(format!("{sequence:08}.json")),
        &UndoAudit {
            schema_version: 1,
            sequence,
            actor: "desktop-user".to_owned(),
            recorded_at_unix_ms: unix_time_ms(SystemTime::now()),
            state,
            invariant_result: invariant_result.to_owned(),
            outcome: outcome.to_owned(),
            failure_reason: failure_reason.map(str::to_owned),
            plan: plan.clone(),
        },
    )
}

fn unix_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateArchiveUndoPlanRequest {
    operation_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConfirmArchiveUndoPlanRequest {
    undo_id: String,
    confirmation_nonce: String,
}

#[tauri::command]
pub fn create_archive_undo_plan(
    request: CreateArchiveUndoPlanRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchiveUndoPlanRegistry>,
) -> Result<ArchiveUndoPlan, String> {
    let summary = vaults.current_summary()?;
    let vault = vaults.lease(&summary.authority_id)?;
    plans.create_at(
        &vault,
        &request.operation_id,
        Instant::now(),
        SystemTime::now(),
    )
}

#[tauri::command]
pub async fn confirm_archive_undo_plan(
    request: ConfirmArchiveUndoPlanRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, ArchiveUndoPlanRegistry>,
) -> Result<ArchiveUndoResult, String> {
    let plan = plans.consume_at(
        &request.undo_id,
        &request.confirmation_nonce,
        Instant::now(),
    )?;
    let vault = vaults.lease(&plan.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        execute_with(&vault, plan, &SystemArchiveUndoExecutor)
    })
    .await
    .map_err(|error| format!("Archive undo transaction worker failed: {error}"))
}

#[cfg(test)]
mod tests;

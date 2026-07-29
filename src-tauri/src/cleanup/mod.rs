mod recovery;

use crate::archive::verified_registered_original;
use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use crate::identity::ContentIdentity;
use crate::vault::records::{read_json, write_new_json};
use crate::vault::{VaultAuthorityRegistry, VaultLease};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PLAN_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_CLEANUP_ITEMS: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupDisposition {
    Trash,
    PermanentDelete,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CleanupPlanItem {
    operation_id: String,
    source_path: String,
    retained_path: String,
    identity: ContentIdentity,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CleanupPlan {
    plan_id: String,
    plan_version: u32,
    authority_id: String,
    disposition: CleanupDisposition,
    items: Vec<CleanupPlanItem>,
    expires_at_unix_ms: u64,
    confirmation_nonce: String,
    confirmation_binding_sha256: String,
}

struct PendingPlan {
    plan: CleanupPlan,
    expires_at: Instant,
}

#[derive(Clone, Default)]
pub struct CleanupPlanRegistry {
    plans: Arc<Mutex<HashMap<String, PendingPlan>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CleanupStatus {
    Committed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    plan_id: String,
    status: CleanupStatus,
    disposition: CleanupDisposition,
    removed_paths: Vec<String>,
    failure_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCleanupPlanRequest {
    authority_id: String,
    operation_ids: Vec<String>,
    cleanup_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizePermanentCleanupRequest {
    plan_id: String,
    confirmation_nonce: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmCleanupPlanRequest {
    plan_id: String,
    confirmation_nonce: String,
}

impl CleanupPlanRegistry {
    fn create_at(
        &self,
        vault: &VaultLease,
        operation_ids: &[String],
        cleanup_enabled: bool,
        now: Instant,
        wall_clock: SystemTime,
    ) -> Result<CleanupPlan, String> {
        if !cleanup_enabled {
            return Err("Cleanup is disabled; enable it explicitly before planning".to_owned());
        }
        if operation_ids.is_empty() || operation_ids.len() > MAX_CLEANUP_ITEMS {
            return Err("Cleanup selection must contain between 1 and 256 items".to_owned());
        }
        let unique = operation_ids.iter().collect::<HashSet<_>>();
        if unique.len() != operation_ids.len() {
            return Err("Cleanup selection contains duplicate archive operations".to_owned());
        }

        let items = operation_ids
            .iter()
            .map(|operation_id| cleanup_item(vault, operation_id))
            .collect::<Result<Vec<_>, _>>()?;
        let plan = new_plan(vault, CleanupDisposition::Trash, items, wall_clock)?;
        persist_state(
            vault,
            &plan,
            0,
            CleanupLifecycleState::Proposed,
            "pending",
            None,
        )?;
        self.insert(plan.clone(), now)?;
        Ok(plan)
    }

    fn escalate_at(
        &self,
        vault: &VaultLease,
        plan_id: &str,
        confirmation_nonce: &str,
        now: Instant,
        wall_clock: SystemTime,
    ) -> Result<CleanupPlan, String> {
        let reviewed = self.consume_at(plan_id, confirmation_nonce, now)?;
        if reviewed.disposition != CleanupDisposition::Trash {
            return Err("Only a reviewed trash plan can request permanent deletion".to_owned());
        }
        reverify_plan(vault, &reviewed)?;
        persist_state(
            vault,
            &reviewed,
            1,
            CleanupLifecycleState::Superseded,
            "permanent-delete-requested",
            None,
        )?;
        let plan = new_plan(
            vault,
            CleanupDisposition::PermanentDelete,
            reviewed.items,
            wall_clock,
        )?;
        persist_state(
            vault,
            &plan,
            0,
            CleanupLifecycleState::Proposed,
            "awaiting-separate-permanent-confirmation",
            None,
        )?;
        self.insert(plan.clone(), now)?;
        Ok(plan)
    }

    fn insert(&self, plan: CleanupPlan, now: Instant) -> Result<(), String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Cleanup plan registry is unavailable".to_owned())?;
        plans.retain(|_, pending| pending.expires_at > now);
        plans.insert(
            plan.plan_id.clone(),
            PendingPlan {
                plan,
                expires_at: now + PLAN_TTL,
            },
        );
        Ok(())
    }

    fn consume_at(
        &self,
        plan_id: &str,
        confirmation_nonce: &str,
        now: Instant,
    ) -> Result<CleanupPlan, String> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| "Cleanup plan registry is unavailable".to_owned())?;
        plans.retain(|_, pending| pending.expires_at > now);
        let pending = plans
            .get(plan_id)
            .ok_or_else(|| "Cleanup plan is missing, expired, or already consumed".to_owned())?;
        if pending.plan.confirmation_nonce != confirmation_nonce {
            return Err("Cleanup confirmation does not match the reviewed plan".to_owned());
        }
        Ok(plans
            .remove(plan_id)
            .expect("validated cleanup plan remains present")
            .plan)
    }
}

fn new_plan(
    vault: &VaultLease,
    disposition: CleanupDisposition,
    items: Vec<CleanupPlanItem>,
    wall_clock: SystemTime,
) -> Result<CleanupPlan, String> {
    let plan_id = Uuid::new_v4().simple().to_string();
    let confirmation_nonce = Uuid::new_v4().simple().to_string();
    let expires_at_unix_ms = unix_time_ms(wall_clock + PLAN_TTL);
    let confirmation_binding_sha256 = confirmation_binding(
        &plan_id,
        &vault.summary.authority_id,
        disposition,
        &items,
        expires_at_unix_ms,
        &confirmation_nonce,
    )?;
    Ok(CleanupPlan {
        plan_id,
        plan_version: 1,
        authority_id: vault.summary.authority_id.clone(),
        disposition,
        items,
        expires_at_unix_ms,
        confirmation_nonce,
        confirmation_binding_sha256,
    })
}

fn cleanup_item(vault: &VaultLease, operation_id: &str) -> Result<CleanupPlanItem, String> {
    let original = verified_registered_original(vault, operation_id)?;
    let source_identity = verified_source_identity(Path::new(&original.source_path))?;
    if source_identity != original.identity {
        return Err("Cleanup source no longer matches the registered original".to_owned());
    }
    let retained_path = Path::new(&vault.summary.display_path)
        .join(&original.relative_path)
        .to_string_lossy()
        .into_owned();
    if retained_path == original.source_path {
        return Err("Cleanup cannot target the retained registered original".to_owned());
    }
    Ok(CleanupPlanItem {
        operation_id: original.operation_id,
        source_path: original.source_path,
        retained_path,
        identity: original.identity,
    })
}

fn verified_source_identity(path: &Path) -> Result<ContentIdentity, String> {
    let file = match open_trusted_drop_root(path.to_path_buf()) {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Cleanup source must be a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { .. } => {
            return Err("Cleanup source is unreadable or no longer trusted".to_owned())
        }
    };
    ContentIdentity::from_reader(file)
        .map_err(|error| format!("Cleanup source identity cannot be computed: {error}"))
}

fn reverify_plan(vault: &VaultLease, plan: &CleanupPlan) -> Result<(), String> {
    if plan.authority_id != vault.summary.authority_id {
        return Err("Cleanup plan belongs to a different Vault authority".to_owned());
    }
    let expected = confirmation_binding(
        &plan.plan_id,
        &plan.authority_id,
        plan.disposition,
        &plan.items,
        plan.expires_at_unix_ms,
        &plan.confirmation_nonce,
    )?;
    if expected != plan.confirmation_binding_sha256 {
        return Err("Cleanup plan binding is invalid".to_owned());
    }
    for item in &plan.items {
        let original = verified_registered_original(vault, &item.operation_id)?;
        if original.identity != item.identity
            || original.source_path != item.source_path
            || verified_source_identity(Path::new(&item.source_path))? != item.identity
        {
            return Err("Cleanup item changed after review".to_owned());
        }
    }
    Ok(())
}

fn confirmation_binding(
    plan_id: &str,
    authority_id: &str,
    disposition: CleanupDisposition,
    items: &[CleanupPlanItem],
    expires_at_unix_ms: u64,
    nonce: &str,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(plan_id.as_bytes());
    hasher.update([0]);
    hasher.update(authority_id.as_bytes());
    hasher.update([0]);
    hasher.update(
        serde_json::to_vec(&(disposition, items, expires_at_unix_ms, nonce))
            .map_err(|error| format!("Cleanup plan cannot be bound: {error}"))?,
    );
    Ok(format!("{:x}", hasher.finalize()))
}

trait CleanupExecutor {
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
    fn delete_permanently(&self, path: &Path) -> Result<(), String>;
}

struct SystemCleanupExecutor;

impl CleanupExecutor for SystemCleanupExecutor {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path)
            .map_err(|_| "Operating-system trash rejected the cleanup item".to_owned())
    }

    fn delete_permanently(&self, path: &Path) -> Result<(), String> {
        fs::remove_file(path).map_err(|_| "Permanent deletion rejected the cleanup item".to_owned())
    }
}

fn execute_with(
    vault: &VaultLease,
    plan: CleanupPlan,
    executor: &dyn CleanupExecutor,
) -> CleanupResult {
    if let Err(error) = reverify_plan(vault, &plan) {
        let _ = persist_state(
            vault,
            &plan,
            1,
            CleanupLifecycleState::Failed,
            "not-mutated",
            Some(&error),
        );
        return failed_result(plan, error);
    }
    if let Err(error) = persist_state(
        vault,
        &plan,
        1,
        CleanupLifecycleState::Executing,
        "retained-original-verified",
        None,
    ) {
        return failed_result(plan, error);
    }
    let mut removed_paths = Vec::new();
    for item in &plan.items {
        let result = match plan.disposition {
            CleanupDisposition::Trash => executor.move_to_trash(Path::new(&item.source_path)),
            CleanupDisposition::PermanentDelete => {
                executor.delete_permanently(Path::new(&item.source_path))
            }
        };
        if let Err(error) = result {
            let audit_result = persist_state(
                vault,
                &plan,
                2,
                CleanupLifecycleState::Failed,
                "retained-original-preserved",
                Some(&error),
            );
            let failure_reason = match audit_result {
                Ok(()) => error,
                Err(audit_error) => {
                    format!("{error}; cleanup outcome audit failed: {audit_error}")
                }
            };
            return CleanupResult {
                plan_id: plan.plan_id,
                status: CleanupStatus::Failed,
                disposition: plan.disposition,
                removed_paths,
                failure_reason: Some(failure_reason),
            };
        }
        removed_paths.push(item.source_path.clone());
    }
    if let Err(error) = persist_state(
        vault,
        &plan,
        2,
        CleanupLifecycleState::Committed,
        "retained-original-verified",
        None,
    ) {
        return CleanupResult {
            plan_id: plan.plan_id,
            status: CleanupStatus::Failed,
            disposition: plan.disposition,
            removed_paths,
            failure_reason: Some(format!(
                "Cleanup completed but outcome audit failed: {error}"
            )),
        };
    }
    CleanupResult {
        plan_id: plan.plan_id,
        status: CleanupStatus::Committed,
        disposition: plan.disposition,
        removed_paths,
        failure_reason: None,
    }
}

fn failed_result(plan: CleanupPlan, error: String) -> CleanupResult {
    CleanupResult {
        plan_id: plan.plan_id,
        status: CleanupStatus::Failed,
        disposition: plan.disposition,
        removed_paths: Vec::new(),
        failure_reason: Some(error),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum CleanupLifecycleState {
    Proposed,
    Executing,
    Superseded,
    Committed,
    Failed,
    Abandoned,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CleanupAudit {
    schema_version: u32,
    sequence: u32,
    actor: String,
    recorded_at_unix_ms: u64,
    state: CleanupLifecycleState,
    outcome: String,
    failure_reason: Option<String>,
    plan: CleanupPlan,
}

fn persist_state(
    vault: &VaultLease,
    plan: &CleanupPlan,
    sequence: u32,
    state: CleanupLifecycleState,
    outcome: &str,
    failure_reason: Option<&str>,
) -> Result<(), String> {
    let directory = Path::new(".aiks/cleanup").join(&plan.plan_id);
    match vault.directory.symlink_metadata(&directory) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err("Cleanup audit path is not a trusted directory".to_owned())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => vault
            .directory
            .create_dir(&directory)
            .map_err(|error| format!("Cleanup audit directory cannot be created: {error}"))?,
        Err(error) => {
            return Err(format!(
                "Cleanup audit directory cannot be inspected: {error}"
            ))
        }
    }
    write_new_json(
        &vault.directory,
        &directory.join(format!("{sequence:08}.json")),
        &CleanupAudit {
            schema_version: 1,
            sequence,
            actor: "desktop-user".to_owned(),
            recorded_at_unix_ms: unix_time_ms(SystemTime::now()),
            state,
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

#[tauri::command]
pub fn create_cleanup_plan(
    request: CreateCleanupPlanRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, CleanupPlanRegistry>,
) -> Result<CleanupPlan, String> {
    let vault = vaults.lease(&request.authority_id)?;
    plans.create_at(
        &vault,
        &request.operation_ids,
        request.cleanup_enabled,
        Instant::now(),
        SystemTime::now(),
    )
}

#[tauri::command]
pub fn authorize_permanent_cleanup(
    request: AuthorizePermanentCleanupRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, CleanupPlanRegistry>,
) -> Result<CleanupPlan, String> {
    let summary = vaults.current_summary()?;
    let vault = vaults.lease(&summary.authority_id)?;
    plans.escalate_at(
        &vault,
        &request.plan_id,
        &request.confirmation_nonce,
        Instant::now(),
        SystemTime::now(),
    )
}

#[tauri::command]
pub async fn confirm_cleanup_plan(
    request: ConfirmCleanupPlanRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    plans: tauri::State<'_, CleanupPlanRegistry>,
) -> Result<CleanupResult, String> {
    let plan = plans.consume_at(
        &request.plan_id,
        &request.confirmation_nonce,
        Instant::now(),
    )?;
    let vault = vaults.lease(&plan.authority_id)?;
    tauri::async_runtime::spawn_blocking(move || execute_with(&vault, plan, &SystemCleanupExecutor))
        .await
        .map_err(|error| format!("Cleanup transaction worker failed: {error}"))
}

pub(crate) use recovery::reconcile_vault;

#[cfg(test)]
mod tests;

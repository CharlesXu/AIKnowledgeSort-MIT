pub(crate) mod records;

use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PRODUCT_DIRECTORIES: [&str; 26] = [
    ".aiks",
    ".aiks/operations",
    ".aiks/archive-audit-anchors",
    ".aiks/vault-transfers",
    ".aiks/registrations",
    ".aiks/staging",
    ".aiks/pending-registrations",
    ".aiks/knowledge",
    ".aiks/comparisons",
    ".aiks/cleanup",
    ".aiks/archive-undo",
    ".aiks/undo-staging",
    ".aiks/undo-trash",
    ".aiks/undone-registrations",
    ".aiks/graph",
    ".aiks/graph/relations",
    ".aiks/file-semantic-comparisons",
    ".aiks/profiles",
    ".aiks/profiles/sources",
    ".aiks/profiles/compiler-sources",
    ".aiks/profiles/candidates",
    ".aiks/profiles/decisions",
    ".aiks/profiles/installed",
    ".aiks/profiles/activations",
    "Originals",
    "Knowledge",
];
const VAULT_AUTHORITY_RECORD: &str = ".aiks/vault-authority.json";
const VAULT_AUTHORITY_SCHEMA_VERSION: u32 = 1;
const VAULT_TRANSFER_SCHEMA_VERSION: u32 = 1;
const VAULT_TRANSFER_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VaultStatus {
    Authoritative,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    pub authority_id: String,
    pub display_path: String,
    pub status: VaultStatus,
}

struct VaultAuthority {
    summary: VaultSummary,
    directory: Dir,
    display_path: PathBuf,
    active_leases: Arc<AtomicUsize>,
}

pub(crate) struct VaultLease {
    pub summary: VaultSummary,
    pub directory: Dir,
    pub(crate) active_counter: Option<Arc<AtomicUsize>>,
}

impl Drop for VaultLease {
    fn drop(&mut self) {
        if let Some(counter) = self.active_counter.as_ref() {
            counter.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl VaultLease {
    pub(crate) fn from_granted_scope(directory: Dir) -> Result<Self, String> {
        let path = Path::new(VAULT_AUTHORITY_RECORD);
        let metadata = directory
            .symlink_metadata(path)
            .map_err(|_| "Granted scope is not an initialized Vault".to_owned())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("Granted Vault authority record is not a regular file".to_owned());
        }
        let record: VaultAuthorityRecord = records::read_json(&directory, path)?;
        if record.schema_version != VAULT_AUTHORITY_SCHEMA_VERSION
            || record.authority_id.is_empty()
            || record.authority_id.len() > 128
        {
            return Err("Granted Vault authority record has an unsupported schema".to_owned());
        }
        Ok(Self {
            summary: VaultSummary {
                authority_id: record.authority_id,
                display_path: "granted-vault-scope".to_owned(),
                status: VaultStatus::Authoritative,
            },
            directory,
            active_counter: None,
        })
    }

    pub(crate) fn occupied_names_for_digest(
        &self,
        digest: &str,
        max_names: usize,
    ) -> Result<Vec<String>, String> {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Invalid digest namespace".to_owned());
        }
        let relative = Path::new("Originals").join(digest);
        match self.directory.symlink_metadata(&relative) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("Vault digest namespace is not a trusted directory".to_owned())
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "Vault digest namespace cannot be inspected: {error}"
                ))
            }
        }

        let entries = self
            .directory
            .read_dir(&relative)
            .map_err(|error| format!("Vault digest namespace cannot be read: {error}"))?;
        let mut names = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Vault digest entry cannot be read: {error}"))?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "Vault digest entry name is not valid UTF-8".to_owned())?;
            names.push(name);
            if names.len() > max_names {
                return Err("Vault digest namespace exceeds the naming limit".to_owned());
            }
        }
        names.sort();
        Ok(names)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultAuthorityRecord {
    schema_version: u32,
    authority_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTransferConfirmation {
    pub transfer_id: String,
    pub confirmation_nonce: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTransferPlan {
    pub schema_version: u32,
    pub transfer_id: String,
    pub from_authority_id: String,
    pub from_display_path: String,
    pub target_display_path: String,
    pub expires_at_unix_ms: u64,
    pub confirmation_nonce: String,
    pub content_migrated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultTransferResult {
    pub transfer_id: String,
    pub previous_authority_id: String,
    pub vault: VaultSummary,
    pub content_migrated: bool,
    pub audit_relative_path: String,
}

struct PendingVaultTransfer {
    plan: VaultTransferPlan,
    target_directory: Dir,
    target_display_path: PathBuf,
    expires_at: Instant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultTransferAudit {
    schema_version: u32,
    transfer_id: String,
    actor: &'static str,
    action: &'static str,
    decision: &'static str,
    from_authority_id: String,
    to_authority_id: String,
    target_display_path: String,
    content_migrated: bool,
    invariant: &'static str,
    outcome: &'static str,
    recorded_at_unix_ms: u64,
}

#[derive(Clone, Default)]
pub struct VaultAuthorityRegistry {
    authority: Arc<Mutex<Option<VaultAuthority>>>,
    pending_transfers: Arc<Mutex<HashMap<String, PendingVaultTransfer>>>,
}

impl VaultAuthorityRegistry {
    pub(crate) fn lease(&self, authority_id: &str) -> Result<VaultLease, String> {
        let authority = self
            .authority
            .lock()
            .map_err(|_| "Vault authority registry is unavailable".to_owned())?;
        let current = authority
            .as_ref()
            .ok_or_else(|| "No authoritative Vault has been selected".to_owned())?;
        if current.summary.authority_id != authority_id {
            return Err("Archive plan Vault authority is no longer current".to_owned());
        }
        let directory = current
            .directory
            .try_clone()
            .map_err(|error| format!("Authoritative Vault capability cannot be cloned: {error}"))?;
        directory
            .metadata(".")
            .map_err(|error| format!("Authoritative Vault is no longer readable: {error}"))?;
        current.active_leases.fetch_add(1, Ordering::AcqRel);
        Ok(VaultLease {
            summary: current.summary.clone(),
            directory,
            active_counter: Some(Arc::clone(&current.active_leases)),
        })
    }

    pub fn current_summary(&self) -> Result<VaultSummary, String> {
        let authority = self
            .authority
            .lock()
            .map_err(|_| "Vault authority registry is unavailable".to_owned())?;
        let current = authority
            .as_ref()
            .ok_or_else(|| "No authoritative Vault has been selected".to_owned())?;
        current
            .directory
            .metadata(".")
            .map_err(|error| format!("Authoritative Vault is no longer readable: {error}"))?;
        Ok(current.summary.clone())
    }

    pub fn authorize_path(&self, path: &Path) -> Result<VaultSummary, String> {
        let mut authority = self
            .authority
            .lock()
            .map_err(|_| "Vault authority registry is unavailable".to_owned())?;
        if let Some(current) = authority.as_ref() {
            if current.display_path == path {
                current.directory.metadata(".").map_err(|error| {
                    format!("Authoritative Vault is no longer readable: {error}")
                })?;
                return Ok(current.summary.clone());
            }
            return Err(
                "A different authoritative Vault already exists; authority transfer is required"
                    .to_owned(),
            );
        }

        let (display_path, directory) = match open_trusted_drop_root(path.to_path_buf()) {
            CapabilityRoot::Directory {
                display_path,
                directory,
            } => (display_path, directory),
            CapabilityRoot::File { .. } => {
                return Err("The authoritative Vault must be a directory".to_owned())
            }
            CapabilityRoot::Diagnostic { message, .. } => return Err(message),
        };
        initialize_product_directories(&directory)?;
        let authority_id = load_or_create_authority_id(&directory)?;
        let summary = VaultSummary {
            authority_id,
            display_path: display_path.to_string_lossy().into_owned(),
            status: VaultStatus::Authoritative,
        };
        let lease = VaultLease {
            summary: summary.clone(),
            directory: directory
                .try_clone()
                .map_err(|error| format!("Vault capability cannot be cloned: {error}"))?,
            active_counter: None,
        };
        crate::archive::reconcile_undo_vault(&lease)?;
        crate::archive::reconcile_vault(&lease)?;
        crate::cleanup::reconcile_vault(&lease)?;
        *authority = Some(VaultAuthority {
            summary: summary.clone(),
            directory,
            display_path,
            active_leases: Arc::new(AtomicUsize::new(0)),
        });
        Ok(summary)
    }

    pub fn prepare_transfer_path(&self, path: &Path) -> Result<VaultTransferPlan, String> {
        let authority = self
            .authority
            .lock()
            .map_err(|_| "Vault authority registry is unavailable".to_owned())?;
        let current = authority
            .as_ref()
            .ok_or_else(|| "No authoritative Vault has been selected".to_owned())?;
        current
            .directory
            .metadata(".")
            .map_err(|error| format!("Authoritative Vault is no longer readable: {error}"))?;

        let (target_display_path, target_directory) =
            match open_trusted_drop_root(path.to_path_buf()) {
                CapabilityRoot::Directory {
                    display_path,
                    directory,
                } => (display_path, directory),
                CapabilityRoot::File { .. } => {
                    return Err("The transfer target must be a directory".to_owned())
                }
                CapabilityRoot::Diagnostic { message, .. } => return Err(message),
            };
        if current.display_path == target_display_path {
            return Err("The transfer target is already authoritative".to_owned());
        }

        let now = Instant::now();
        let transfer_id = Uuid::new_v4().simple().to_string();
        let plan = VaultTransferPlan {
            schema_version: VAULT_TRANSFER_SCHEMA_VERSION,
            transfer_id: transfer_id.clone(),
            from_authority_id: current.summary.authority_id.clone(),
            from_display_path: current.summary.display_path.clone(),
            target_display_path: target_display_path.to_string_lossy().into_owned(),
            expires_at_unix_ms: unix_time_ms(SystemTime::now() + VAULT_TRANSFER_TTL),
            confirmation_nonce: Uuid::new_v4().simple().to_string(),
            content_migrated: false,
        };
        let mut pending = self
            .pending_transfers
            .lock()
            .map_err(|_| "Vault transfer registry is unavailable".to_owned())?;
        pending.clear();
        pending.insert(
            transfer_id,
            PendingVaultTransfer {
                plan: plan.clone(),
                target_directory,
                target_display_path,
                expires_at: now + VAULT_TRANSFER_TTL,
            },
        );
        Ok(plan)
    }

    pub fn confirm_transfer(
        &self,
        confirmation: VaultTransferConfirmation,
    ) -> Result<VaultTransferResult, String> {
        let mut authority = self
            .authority
            .lock()
            .map_err(|_| "Vault authority registry is unavailable".to_owned())?;
        let current = authority
            .as_ref()
            .ok_or_else(|| "No authoritative Vault has been selected".to_owned())?;
        let mut pending = self
            .pending_transfers
            .lock()
            .map_err(|_| "Vault transfer registry is unavailable".to_owned())?;
        let transfer = pending
            .get(&confirmation.transfer_id)
            .ok_or_else(|| "Vault transfer plan is missing or already consumed".to_owned())?;
        if transfer.expires_at <= Instant::now() {
            pending.remove(&confirmation.transfer_id);
            return Err("Vault transfer plan has expired".to_owned());
        }
        if transfer.plan.confirmation_nonce != confirmation.confirmation_nonce {
            return Err("Vault transfer confirmation does not match the plan".to_owned());
        }
        if transfer.plan.from_authority_id != current.summary.authority_id
            || transfer.plan.from_display_path != current.summary.display_path
        {
            return Err("Vault authority changed after the transfer was prepared".to_owned());
        }
        if current.active_leases.load(Ordering::Acquire) != 0 {
            return Err("Vault authority cannot transfer while operations are active".to_owned());
        }
        let transfer = pending
            .remove(&confirmation.transfer_id)
            .ok_or_else(|| "Vault transfer plan is unavailable".to_owned())?;
        drop(pending);

        initialize_product_directories(&transfer.target_directory)?;
        let target_authority_id = load_or_create_authority_id(&transfer.target_directory)?;
        if target_authority_id == current.summary.authority_id {
            return Err("Transfer target duplicates the current Vault authority".to_owned());
        }
        let target_summary = VaultSummary {
            authority_id: target_authority_id,
            display_path: transfer.plan.target_display_path.clone(),
            status: VaultStatus::Authoritative,
        };
        let target_lease = VaultLease {
            summary: target_summary.clone(),
            directory: transfer
                .target_directory
                .try_clone()
                .map_err(|error| format!("Target Vault capability cannot be cloned: {error}"))?,
            active_counter: None,
        };
        crate::archive::reconcile_undo_vault(&target_lease)?;
        crate::archive::reconcile_vault(&target_lease)?;
        crate::cleanup::reconcile_vault(&target_lease)?;

        let audit_relative_path =
            format!(".aiks/vault-transfers/{}.json", transfer.plan.transfer_id);
        records::write_new_json(
            &current.directory,
            Path::new(&audit_relative_path),
            &VaultTransferAudit {
                schema_version: VAULT_TRANSFER_SCHEMA_VERSION,
                transfer_id: transfer.plan.transfer_id.clone(),
                actor: "desktop-user",
                action: "authorityTransfer",
                decision: "confirmed",
                from_authority_id: current.summary.authority_id.clone(),
                to_authority_id: target_summary.authority_id.clone(),
                target_display_path: transfer.plan.target_display_path,
                content_migrated: false,
                invariant: "single-authoritative-vault",
                outcome: "committed",
                recorded_at_unix_ms: unix_time_ms(SystemTime::now()),
            },
        )?;

        let previous_authority_id = current.summary.authority_id.clone();
        *authority = Some(VaultAuthority {
            summary: target_summary.clone(),
            directory: transfer.target_directory,
            display_path: transfer.target_display_path,
            active_leases: Arc::new(AtomicUsize::new(0)),
        });
        Ok(VaultTransferResult {
            transfer_id: transfer.plan.transfer_id,
            previous_authority_id,
            vault: target_summary,
            content_migrated: false,
            audit_relative_path,
        })
    }
}

fn unix_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn load_or_create_authority_id(directory: &Dir) -> Result<String, String> {
    let path = Path::new(VAULT_AUTHORITY_RECORD);
    match directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Vault authority record is not a regular file".to_owned())
        }
        Ok(_) => {
            let record: VaultAuthorityRecord = records::read_json(directory, path)?;
            if record.schema_version != VAULT_AUTHORITY_SCHEMA_VERSION
                || record.authority_id.is_empty()
                || record.authority_id.len() > 128
            {
                return Err("Vault authority record has an unsupported schema".to_owned());
            }
            Ok(record.authority_id)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let record = VaultAuthorityRecord {
                schema_version: VAULT_AUTHORITY_SCHEMA_VERSION,
                authority_id: Uuid::new_v4().simple().to_string(),
            };
            records::write_new_json(directory, path, &record)?;
            Ok(record.authority_id)
        }
        Err(error) => Err(format!(
            "Vault authority record cannot be inspected: {error}"
        )),
    }
}

fn initialize_product_directories(directory: &Dir) -> Result<(), String> {
    for relative in PRODUCT_DIRECTORIES {
        let path = Path::new(relative);
        match directory.symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!("Vault product directory is a link: {relative}"))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!("Vault product path is not a directory: {relative}"))
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                directory.create_dir(path).map_err(|error| {
                    format!("Vault product directory cannot be created: {error}")
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "Vault product directory cannot be inspected: {error}"
                ))
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn choose_authoritative_vault(
    app: tauri::AppHandle,
    registry: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<Option<VaultSummary>, String> {
    let selected = crate::native_dialog::pick_folder(&app).await?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected Vault path is unavailable: {error}"))?;
    registry.authorize_path(&path).map(Some)
}

#[tauri::command]
pub async fn prepare_authority_transfer(
    app: tauri::AppHandle,
    registry: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<Option<VaultTransferPlan>, String> {
    let selected = crate::native_dialog::pick_folder(&app).await?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected transfer target is unavailable: {error}"))?;
    registry.prepare_transfer_path(&path).map(Some)
}

#[tauri::command]
pub fn confirm_authority_transfer(
    registry: tauri::State<'_, VaultAuthorityRegistry>,
    request: VaultTransferConfirmation,
) -> Result<VaultTransferResult, String> {
    registry.confirm_transfer(request)
}

#[cfg(test)]
mod tests {
    use super::{VaultAuthorityRegistry, VaultTransferConfirmation};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new() -> Self {
            let unique = format!(
                "aiknowledgesort-vault-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir(&root).expect("create generated temporary tree");
            Self {
                root: root.canonicalize().expect("canonical temporary tree"),
            }
        }

        fn directory(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::create_dir(&path).expect("create generated directory");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove generated temporary tree");
        }
    }

    fn assert_product_directories(vault: &Path) {
        for relative in [
            ".aiks/operations",
            ".aiks/registrations",
            ".aiks/staging",
            ".aiks/pending-registrations",
            ".aiks/profiles/sources",
            ".aiks/profiles/candidates",
            ".aiks/profiles/decisions",
            ".aiks/profiles/installed",
            ".aiks/profiles/activations",
            ".aiks/comparisons",
            ".aiks/cleanup",
            "Originals",
        ] {
            assert!(vault.join(relative).is_dir(), "{relative} must exist");
        }
    }

    #[test]
    fn establishes_one_idempotent_authoritative_vault() {
        let tree = TempTree::new();
        let first = tree.directory("first");
        let second = tree.directory("second");
        let registry = VaultAuthorityRegistry::default();

        let initial = registry
            .authorize_path(&first)
            .expect("authorize first Vault");
        let repeated = registry.authorize_path(&first).expect("repeat same Vault");

        assert_eq!(initial, repeated);
        assert_product_directories(&first);
        assert!(registry.authorize_path(&second).is_err());
        assert!(second.read_dir().expect("read second").next().is_none());
    }

    #[test]
    fn transfers_the_single_authority_only_after_exact_confirmation() {
        let tree = TempTree::new();
        let first = tree.directory("first");
        let second = tree.directory("second");
        let registry = VaultAuthorityRegistry::default();
        let initial = registry
            .authorize_path(&first)
            .expect("authorize first Vault");
        fs::write(first.join("Originals").join("retained.txt"), b"source")
            .expect("write retained source");

        let plan = registry
            .prepare_transfer_path(&second)
            .expect("prepare transfer");
        assert_eq!(plan.from_authority_id, initial.authority_id);
        assert!(!plan.content_migrated);
        assert!(second.read_dir().expect("read target").next().is_none());
        assert_eq!(registry.current_summary().expect("current Vault"), initial);

        assert!(registry
            .confirm_transfer(VaultTransferConfirmation {
                transfer_id: plan.transfer_id.clone(),
                confirmation_nonce: "wrong-nonce".to_owned(),
            })
            .is_err());
        let result = registry
            .confirm_transfer(VaultTransferConfirmation {
                transfer_id: plan.transfer_id.clone(),
                confirmation_nonce: plan.confirmation_nonce,
            })
            .expect("confirm transfer");

        assert_eq!(result.previous_authority_id, initial.authority_id);
        assert_eq!(result.vault, registry.current_summary().expect("new Vault"));
        assert!(!result.content_migrated);
        assert!(first.join("Originals").join("retained.txt").is_file());
        assert!(second.join(".aiks/vault-authority.json").is_file());
        assert!(first.join(&result.audit_relative_path).is_file());
        let audit: serde_json::Value = serde_json::from_slice(
            &fs::read(first.join(&result.audit_relative_path)).expect("read transfer audit"),
        )
        .expect("parse transfer audit");
        assert_eq!(audit["transferId"], result.transfer_id);
        assert_eq!(audit["fromAuthorityId"], initial.authority_id);
        assert_eq!(audit["toAuthorityId"], result.vault.authority_id);
        assert_eq!(audit["actor"], "desktop-user");
        assert_eq!(audit["action"], "authorityTransfer");
        assert_eq!(audit["contentMigrated"], false);
        assert!(registry.lease(&initial.authority_id).is_err());
        assert!(registry.lease(&result.vault.authority_id).is_ok());
        assert!(registry
            .confirm_transfer(VaultTransferConfirmation {
                transfer_id: result.transfer_id,
                confirmation_nonce: "replay".to_owned(),
            })
            .is_err());
    }

    #[test]
    fn blocks_transfer_while_the_current_vault_has_an_active_lease() {
        let tree = TempTree::new();
        let first = tree.directory("first");
        let second = tree.directory("second");
        let registry = VaultAuthorityRegistry::default();
        let initial = registry
            .authorize_path(&first)
            .expect("authorize first Vault");
        let plan = registry
            .prepare_transfer_path(&second)
            .expect("prepare transfer");
        let active = registry
            .lease(&initial.authority_id)
            .expect("lease current Vault");
        let confirmation = VaultTransferConfirmation {
            transfer_id: plan.transfer_id,
            confirmation_nonce: plan.confirmation_nonce,
        };

        assert!(registry.confirm_transfer(confirmation.clone()).is_err());
        assert_eq!(registry.current_summary().expect("current Vault"), initial);
        assert!(second.read_dir().expect("read target").next().is_none());

        drop(active);
        registry
            .confirm_transfer(confirmation)
            .expect("confirm after lease closes");
    }

    #[test]
    fn keeps_the_current_authority_when_transfer_audit_cannot_commit() {
        let tree = TempTree::new();
        let first = tree.directory("first");
        let second = tree.directory("second");
        let registry = VaultAuthorityRegistry::default();
        let initial = registry
            .authorize_path(&first)
            .expect("authorize first Vault");
        let plan = registry
            .prepare_transfer_path(&second)
            .expect("prepare transfer");
        fs::remove_dir(first.join(".aiks/vault-transfers"))
            .expect("remove transfer audit directory");
        fs::write(first.join(".aiks/vault-transfers"), b"blocked")
            .expect("block transfer audit path");

        assert!(registry
            .confirm_transfer(VaultTransferConfirmation {
                transfer_id: plan.transfer_id,
                confirmation_nonce: plan.confirmation_nonce,
            })
            .is_err());
        assert_eq!(registry.current_summary().expect("current Vault"), initial);
        assert!(registry.lease(&initial.authority_id).is_ok());
    }

    #[test]
    fn reads_the_actual_bounded_digest_namespace_for_name_collisions() {
        let tree = TempTree::new();
        let vault = tree.directory("vault");
        let registry = VaultAuthorityRegistry::default();
        let summary = registry.authorize_path(&vault).expect("authorize Vault");
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let namespace = vault.join("Originals").join(digest);
        fs::create_dir(&namespace).expect("create digest namespace");
        fs::write(namespace.join("Report.pdf"), b"one").expect("write occupied name");
        fs::write(namespace.join("Other.pdf"), b"two").expect("write second occupied name");

        let lease = registry.lease(&summary.authority_id).expect("lease Vault");
        assert_eq!(
            lease
                .occupied_names_for_digest(digest, 2)
                .expect("read occupied names"),
            vec!["Other.pdf".to_owned(), "Report.pdf".to_owned()]
        );
        assert!(lease.occupied_names_for_digest(digest, 1).is_err());
        assert_eq!(
            lease
                .occupied_names_for_digest(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    2,
                )
                .expect("missing namespace"),
            Vec::<String>::new()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_as_vault_authority() {
        let tree = TempTree::new();
        let real = tree.directory("real");
        let alias = tree.root.join("alias");
        std::os::unix::fs::symlink(real, &alias).expect("create generated symlink");
        let registry = VaultAuthorityRegistry::default();

        assert!(registry.authorize_path(&alias).is_err());
    }
}

pub(crate) mod records;

use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const PRODUCT_DIRECTORIES: [&str; 6] = [
    ".aiks",
    ".aiks/operations",
    ".aiks/registrations",
    ".aiks/staging",
    ".aiks/pending-registrations",
    "Originals",
];
const VAULT_AUTHORITY_RECORD: &str = ".aiks/vault-authority.json";
const VAULT_AUTHORITY_SCHEMA_VERSION: u32 = 1;

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
}

pub(crate) struct VaultLease {
    pub summary: VaultSummary,
    pub directory: Dir,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultAuthorityRecord {
    schema_version: u32,
    authority_id: String,
}

#[derive(Clone, Default)]
pub struct VaultAuthorityRegistry {
    authority: Arc<Mutex<Option<VaultAuthority>>>,
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
        Ok(VaultLease {
            summary: current.summary.clone(),
            directory,
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
        };
        crate::archive::reconcile_vault(&lease)?;
        *authority = Some(VaultAuthority {
            summary: summary.clone(),
            directory,
            display_path,
        });
        Ok(summary)
    }
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
pub fn choose_authoritative_vault(
    app: tauri::AppHandle,
    registry: tauri::State<'_, VaultAuthorityRegistry>,
) -> Result<Option<VaultSummary>, String> {
    use tauri_plugin_dialog::DialogExt;

    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected Vault path is unavailable: {error}"))?;
    registry.authorize_path(&path).map(Some)
}

#[cfg(test)]
mod tests {
    use super::VaultAuthorityRegistry;
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

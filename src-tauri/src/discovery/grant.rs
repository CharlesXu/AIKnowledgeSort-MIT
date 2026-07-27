use super::DiagnosticCategory;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, FileType, OpenOptions};
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_GRANT_ID_BYTES: usize = 128;
pub(crate) const DROP_GRANT_EVENT: &str = "local-drop-grant";
pub(crate) const DROP_GRANT_ERROR_EVENT: &str = "local-drop-grant-error";

#[derive(Clone, Copy)]
pub(super) struct RegistryLimits {
    pub max_grants: usize,
    pub max_paths: usize,
    pub max_path_bytes: usize,
    pub ttl: Duration,
}

impl Default for RegistryLimits {
    fn default() -> Self {
        Self {
            max_grants: 32,
            max_paths: 64,
            max_path_bytes: super::MAX_PATH_BYTES,
            ttl: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DropGrantIssued {
    pub grant_id: String,
}

pub(crate) struct DropGrantRegistry {
    limits: RegistryLimits,
    grants: Mutex<HashMap<String, DropGrant>>,
}

impl Default for DropGrantRegistry {
    fn default() -> Self {
        Self::new(RegistryLimits::default())
    }
}

impl DropGrantRegistry {
    pub(super) fn new(limits: RegistryLimits) -> Self {
        Self {
            limits,
            grants: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn issue_at(
        &self,
        paths: Vec<PathBuf>,
        now: Instant,
    ) -> Result<DropGrantIssued, String> {
        validate_paths(&paths, self.limits)?;
        let mut paths = paths;
        paths.sort();
        paths.dedup();
        let roots = paths
            .into_iter()
            .map(open_trusted_drop_root)
            .collect::<Vec<_>>();

        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Drop grant registry is unavailable".to_owned())?;
        grants.retain(|_, grant| grant.expires_at > now);
        if grants.len() >= self.limits.max_grants {
            return Err("Too many unconsumed drop grants".to_owned());
        }
        let grant_id = unique_grant_id(&grants);
        grants.insert(
            grant_id.clone(),
            DropGrant {
                expires_at: now + self.limits.ttl,
                roots,
            },
        );
        Ok(DropGrantIssued { grant_id })
    }

    pub(super) fn consume_at(&self, grant_id: &str, now: Instant) -> Result<DropGrant, String> {
        if grant_id.is_empty() || grant_id.len() > MAX_GRANT_ID_BYTES {
            return Err("Invalid drop grant id".to_owned());
        }
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Drop grant registry is unavailable".to_owned())?;
        let grant = grants
            .remove(grant_id)
            .ok_or_else(|| "Unknown or already consumed drop grant".to_owned())?;
        if grant.expires_at <= now {
            return Err("Drop grant has expired".to_owned());
        }
        Ok(grant)
    }
}

pub(crate) fn issue_drop_grant(
    registry: &DropGrantRegistry,
    paths: Vec<PathBuf>,
) -> Result<DropGrantIssued, String> {
    registry.issue_at(paths, Instant::now())
}

pub(super) struct DropGrant {
    expires_at: Instant,
    pub roots: Vec<CapabilityRoot>,
}

pub(super) enum CapabilityRoot {
    Directory {
        display_path: PathBuf,
        directory: Dir,
    },
    File {
        display_path: PathBuf,
        file: File,
    },
    Diagnostic {
        display_path: PathBuf,
        category: DiagnosticCategory,
        message: String,
    },
}

fn validate_paths(paths: &[PathBuf], limits: RegistryLimits) -> Result<(), String> {
    if paths.is_empty() {
        return Err("The operating-system drop contained no local paths".to_owned());
    }
    if paths.len() > limits.max_paths {
        return Err("The operating-system drop contains too many paths".to_owned());
    }
    for path in paths {
        if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
            return Err("Dropped paths must be absolute and traversal-free".to_owned());
        }
        if path_display_len(path) > limits.max_path_bytes {
            return Err("A dropped path exceeds the configured length limit".to_owned());
        }
    }
    Ok(())
}

fn unique_grant_id(grants: &HashMap<String, DropGrant>) -> String {
    loop {
        let candidate = Uuid::new_v4().simple().to_string();
        if !grants.contains_key(&candidate) {
            return candidate;
        }
    }
}

pub(super) fn path_display_len(path: &Path) -> usize {
    path.to_string_lossy().len()
}

pub(super) fn read_only_nofollow_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .follow(FollowSymlinks::No)
        .maybe_dir(true);
    options
}

fn open_nofollow(directory: &Dir, path: &Path) -> io::Result<File> {
    directory.open_with(path, &read_only_nofollow_options())
}

fn root_diagnostic(
    display_path: PathBuf,
    category: DiagnosticCategory,
    message: impl Into<String>,
) -> CapabilityRoot {
    CapabilityRoot::Diagnostic {
        display_path,
        category,
        message: message.into(),
    }
}

fn open_trusted_drop_root(display_path: PathBuf) -> CapabilityRoot {
    let Some(name) = display_path.file_name() else {
        return open_filesystem_root(display_path);
    };
    let Some(parent_path) = display_path.parent() else {
        return root_diagnostic(
            display_path,
            DiagnosticCategory::Excluded,
            "Dropped root has no accessible parent",
        );
    };
    let parent = match Dir::open_ambient_dir(parent_path, ambient_authority()) {
        Ok(parent) => parent,
        Err(error) => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped root parent is unreadable: {error}"),
            )
        }
    };
    let relative = Path::new(name);
    let metadata = match parent.symlink_metadata(relative) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Excluded,
                "Dropped path does not exist",
            )
        }
        Err(error) => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped root metadata is unreadable: {error}"),
            )
        }
    };
    if file_type_is_link(&metadata.file_type()) {
        return root_diagnostic(
            display_path,
            DiagnosticCategory::Symlink,
            "Dropped root is a link or reparse point",
        );
    }
    let opened = match open_nofollow(&parent, relative) {
        Ok(opened) => opened,
        Err(error) => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped root cannot be opened without links: {error}"),
            )
        }
    };
    classify_opened_root(display_path, opened)
}

fn open_filesystem_root(display_path: PathBuf) -> CapabilityRoot {
    match Dir::open_ambient_dir(&display_path, ambient_authority()) {
        Ok(directory) => CapabilityRoot::Directory {
            display_path,
            directory,
        },
        Err(error) => root_diagnostic(
            display_path,
            DiagnosticCategory::Unreadable,
            format!("Dropped filesystem root is unreadable: {error}"),
        ),
    }
}

fn classify_opened_root(display_path: PathBuf, opened: File) -> CapabilityRoot {
    let metadata = match opened.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped root handle metadata is unreadable: {error}"),
            )
        }
    };
    if file_type_is_link(&metadata.file_type()) {
        return root_diagnostic(
            display_path,
            DiagnosticCategory::Symlink,
            "Dropped root resolved to a link or reparse point",
        );
    }
    if metadata.is_dir() {
        match Dir::reopen_dir(&opened) {
            Ok(directory) => CapabilityRoot::Directory {
                display_path,
                directory,
            },
            Err(error) => root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped directory capability cannot be opened: {error}"),
            ),
        }
    } else if metadata.is_file() {
        CapabilityRoot::File {
            display_path,
            file: opened,
        }
    } else {
        root_diagnostic(
            display_path,
            DiagnosticCategory::Excluded,
            "Dropped root is not a regular file or directory",
        )
    }
}

pub(super) fn file_type_is_link(file_type: &FileType) -> bool {
    if file_type.is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use cap_std::fs::FileTypeExt;
        if file_type.is_symlink_dir() || file_type.is_symlink_file() {
            return true;
        }
    }
    false
}

use super::DiagnosticCategory;
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt, OpenOptionsMaybeDirExt};
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, File, FileType, OpenOptions};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
#[cfg(windows)]
use std::path::Prefix;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
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
pub struct DropGrantIssued {
    pub grant_id: String,
}

#[derive(Clone)]
pub(crate) struct DropGrantRegistry {
    limits: RegistryLimits,
    grants: Arc<Mutex<HashMap<String, DropGrant>>>,
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
            grants: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(super) fn issue_with_deadline(
        &self,
        paths: Vec<PathBuf>,
        deadline: Instant,
    ) -> Result<DropGrantIssued, String> {
        self.issue_with_deadline_inner(paths, deadline, || {})
    }

    fn issue_with_deadline_inner(
        &self,
        paths: Vec<PathBuf>,
        deadline: Instant,
        before_insert: impl FnOnce(),
    ) -> Result<DropGrantIssued, String> {
        validate_paths(&paths, self.limits)?;
        let mut paths = paths;
        paths.sort();
        paths.dedup();
        let mut roots = Vec::with_capacity(paths.len());
        for path in paths {
            ensure_before_deadline(deadline)?;
            roots.push(open_trusted_drop_root(path));
            ensure_before_deadline(deadline)?;
        }

        before_insert();
        ensure_before_deadline(deadline)?;
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| "Drop grant registry is unavailable".to_owned())?;
        let locked_at = Instant::now();
        ensure_before_deadline_at(deadline, locked_at)?;
        grants.retain(|_, grant| grant.expires_at > locked_at);
        if grants.len() >= self.limits.max_grants {
            return Err("Too many unconsumed drop grants".to_owned());
        }
        let grant_id = unique_grant_id(&grants);
        let insertion_time = Instant::now();
        ensure_before_deadline_at(deadline, insertion_time)?;
        grants.insert(
            grant_id.clone(),
            DropGrant {
                expires_at: insertion_time + self.limits.ttl,
                roots,
            },
        );
        Ok(DropGrantIssued { grant_id })
    }

    #[cfg(test)]
    pub(super) fn issue_at(
        &self,
        paths: Vec<PathBuf>,
        now: Instant,
    ) -> Result<DropGrantIssued, String> {
        self.issue_with_deadline(paths, now + super::DROP_WORK_TIMEOUT)
    }

    #[cfg(test)]
    pub(super) fn issue_with_deadline_and_hook(
        &self,
        paths: Vec<PathBuf>,
        deadline: Instant,
        before_insert: impl FnOnce(),
    ) -> Result<DropGrantIssued, String> {
        self.issue_with_deadline_inner(paths, deadline, before_insert)
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
    deadline: Instant,
) -> Result<DropGrantIssued, String> {
    registry.issue_with_deadline(paths, deadline)
}

fn ensure_before_deadline(deadline: Instant) -> Result<(), String> {
    ensure_before_deadline_at(deadline, Instant::now())
}

fn ensure_before_deadline_at(deadline: Instant, now: Instant) -> Result<(), String> {
    if now >= deadline {
        Err("Drop grant processing deadline exceeded".to_owned())
    } else {
        Ok(())
    }
}

pub(super) struct DropGrant {
    expires_at: Instant,
    pub roots: Vec<CapabilityRoot>,
}

pub(crate) enum CapabilityRoot {
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
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
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

pub(crate) fn open_trusted_drop_root(display_path: PathBuf) -> CapabilityRoot {
    let (filesystem_root, components) = match absolute_capability_path(&display_path) {
        Some(parts) => parts,
        None => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Excluded,
                "Dropped root is not an absolute traversal-free path",
            )
        }
    };
    let mut directory = match Dir::open_ambient_dir(&filesystem_root, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) => {
            return root_diagnostic(
                display_path,
                DiagnosticCategory::Unreadable,
                format!("Dropped filesystem root is unreadable: {error}"),
            )
        }
    };
    if components.is_empty() {
        return CapabilityRoot::Directory {
            display_path,
            directory,
        };
    }

    let (name, ancestors) = components
        .split_last()
        .expect("non-empty absolute path components");
    for ancestor in ancestors {
        directory = match open_ancestor_directory(&directory, ancestor) {
            Ok(directory) => directory,
            Err((category, message)) => return root_diagnostic(display_path, category, message),
        };
    }

    let relative = Path::new(name);
    let metadata = match directory.symlink_metadata(relative) {
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
    if !metadata.is_file() && !metadata.is_dir() {
        return root_diagnostic(
            display_path,
            DiagnosticCategory::Excluded,
            "Dropped root is not a regular file or directory",
        );
    }
    let opened = match open_nofollow(&directory, relative) {
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

fn absolute_capability_path(path: &Path) -> Option<(PathBuf, Vec<OsString>)> {
    let mut components = path.components();
    let mut filesystem_root = PathBuf::new();

    match components.next()? {
        Component::RootDir => filesystem_root.push(std::path::MAIN_SEPARATOR_STR),
        Component::Prefix(prefix) => {
            #[cfg(windows)]
            if matches!(prefix.kind(), Prefix::DeviceNS(_) | Prefix::Verbatim(_)) {
                return None;
            }
            filesystem_root.push(prefix.as_os_str());
            if components.next()? != Component::RootDir {
                return None;
            }
            filesystem_root.push(std::path::MAIN_SEPARATOR_STR);
        }
        _ => return None,
    }

    let mut relative_components = Vec::new();
    for component in components {
        match component {
            Component::Normal(component) => relative_components.push(component.to_os_string()),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => return None,
        }
    }
    Some((filesystem_root, relative_components))
}

fn open_ancestor_directory(
    directory: &Dir,
    name: &OsString,
) -> Result<Dir, (DiagnosticCategory, String)> {
    let relative = Path::new(name);
    let metadata = match directory.symlink_metadata(relative) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err((
                DiagnosticCategory::Excluded,
                "Dropped path ancestor does not exist".to_owned(),
            ))
        }
        Err(error) => {
            return Err((
                DiagnosticCategory::Unreadable,
                format!("Dropped path ancestor metadata is unreadable: {error}"),
            ))
        }
    };
    if file_type_is_link(&metadata.file_type()) {
        return Err((
            DiagnosticCategory::Symlink,
            "Dropped path contains a link or reparse-point ancestor".to_owned(),
        ));
    }
    if !metadata.is_dir() {
        return Err((
            DiagnosticCategory::Excluded,
            "Dropped path ancestor is not a directory".to_owned(),
        ));
    }
    let opened = match open_nofollow(directory, relative) {
        Ok(opened) => opened,
        Err(error) => {
            return Err((
                DiagnosticCategory::Unreadable,
                format!("Dropped path ancestor cannot be opened without links: {error}"),
            ))
        }
    };
    let opened_metadata = opened.metadata().map_err(|error| {
        (
            DiagnosticCategory::Unreadable,
            format!("Dropped path ancestor handle metadata is unreadable: {error}"),
        )
    })?;
    if file_type_is_link(&opened_metadata.file_type()) {
        return Err((
            DiagnosticCategory::Symlink,
            "Dropped path ancestor resolved to a link or reparse point".to_owned(),
        ));
    }
    if !opened_metadata.is_dir() {
        return Err((
            DiagnosticCategory::Excluded,
            "Dropped path ancestor is not a directory".to_owned(),
        ));
    }
    Dir::reopen_dir(&opened).map_err(|error| {
        (
            DiagnosticCategory::Unreadable,
            format!("Dropped path ancestor capability cannot be opened: {error}"),
        )
    })
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
    file_type.is_symlink()
}

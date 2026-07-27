use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

const MAX_DISCOVERY_ITEMS: usize = 10_000;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProposal {
    pub items: Vec<DiscoveredItem>,
    pub counts: DiscoveryCounts,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredItem {
    pub path: String,
    pub name: String,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCounts {
    pub included: usize,
    pub excluded: usize,
    pub unreadable: usize,
    pub symlink: usize,
    pub out_of_scope: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDiagnostic {
    pub category: DiagnosticCategory,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticCategory {
    Excluded,
    Unreadable,
    Symlink,
    OutOfScope,
    TraversalLimit,
}

#[derive(Clone, Debug)]
struct GrantedRoot {
    original: PathBuf,
    canonical: PathBuf,
}

struct DiscoveryState<'a, F> {
    grants: &'a [GrantedRoot],
    max_items: usize,
    readability_probe: &'a F,
    visited_count: usize,
    limit_reported: bool,
    seen_paths: BTreeSet<PathBuf>,
    seen_directories: BTreeSet<PathBuf>,
    seen_files: BTreeSet<PathBuf>,
    proposal: DiscoveryProposal,
}

impl<F> DiscoveryState<'_, F>
where
    F: Fn(&Path) -> io::Result<()>,
{
    fn inspect(&mut self, path: PathBuf) {
        if !self.seen_paths.insert(path.clone()) {
            return;
        }
        if self.visited_count >= self.max_items {
            self.report_limit(&path);
            return;
        }
        self.visited_count += 1;

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.diagnostic(
                    DiagnosticCategory::Excluded,
                    &path,
                    "Path does not exist",
                );
                return;
            }
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &path,
                    format!("Metadata is unreadable: {error}"),
                );
                return;
            }
        };

        if metadata.file_type().is_symlink() {
            self.diagnostic(
                DiagnosticCategory::Symlink,
                &path,
                "Symbolic links are excluded from discovery",
            );
            return;
        }

        let canonical = match fs::canonicalize(&path) {
            Ok(canonical) => canonical,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &path,
                    format!("Path cannot be resolved: {error}"),
                );
                return;
            }
        };

        if !self
            .grants
            .iter()
            .any(|grant| canonical.starts_with(&grant.canonical))
        {
            self.diagnostic(
                DiagnosticCategory::OutOfScope,
                &path,
                "Resolved path is outside the active grant",
            );
            return;
        }

        if self.has_linked_route(&path, &canonical) {
            self.diagnostic(
                DiagnosticCategory::Symlink,
                &path,
                "Path reaches the active grant through a symbolic link",
            );
            return;
        }

        if metadata.is_dir() {
            self.inspect_directory(path, canonical);
        } else if metadata.is_file() {
            self.inspect_file(path, canonical, metadata.len());
        } else {
            self.diagnostic(
                DiagnosticCategory::Excluded,
                &path,
                "Unsupported filesystem item",
            );
        }
    }

    fn inspect_directory(&mut self, path: PathBuf, canonical: PathBuf) {
        if !self.seen_directories.insert(canonical) {
            return;
        }

        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &path,
                    format!("Directory is unreadable: {error}"),
                );
                return;
            }
        };

        let mut children = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => children.push(entry.path()),
                Err(error) => self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &path,
                    format!("Directory entry is unreadable: {error}"),
                ),
            }
        }
        children.sort();
        for child in children {
            self.inspect(child);
        }
    }

    fn inspect_file(&mut self, path: PathBuf, canonical: PathBuf, byte_size: u64) {
        if !self.seen_files.insert(canonical.clone()) {
            return;
        }
        if let Err(error) = (self.readability_probe)(&path) {
            self.diagnostic(
                DiagnosticCategory::Unreadable,
                &path,
                format!("File is unreadable: {error}"),
            );
            return;
        }

        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| canonical.to_string_lossy().into_owned());
        self.proposal.items.push(DiscoveredItem {
            path: canonical.to_string_lossy().into_owned(),
            name,
            byte_size,
        });
        self.proposal.counts.included += 1;
    }

    fn has_linked_route(&self, path: &Path, canonical: &Path) -> bool {
        let mut matches_original_grant = false;
        for grant in self
            .grants
            .iter()
            .filter(|grant| canonical.starts_with(&grant.canonical))
        {
            let Ok(relative) = path.strip_prefix(&grant.original) else {
                continue;
            };
            matches_original_grant = true;
            let mut current = grant.original.clone();
            for component in relative.components() {
                current.push(component);
                if fs::symlink_metadata(&current)
                    .map(|metadata| metadata.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        !matches_original_grant
    }

    fn report_limit(&mut self, path: &Path) {
        if self.limit_reported {
            return;
        }
        self.limit_reported = true;
        self.diagnostic(
            DiagnosticCategory::TraversalLimit,
            path,
            format!("Discovery item limit ({}) was reached", self.max_items),
        );
    }

    fn diagnostic(
        &mut self,
        category: DiagnosticCategory,
        path: &Path,
        message: impl Into<String>,
    ) {
        match category {
            DiagnosticCategory::Excluded | DiagnosticCategory::TraversalLimit => {
                self.proposal.counts.excluded += 1;
            }
            DiagnosticCategory::Unreadable => self.proposal.counts.unreadable += 1,
            DiagnosticCategory::Symlink => self.proposal.counts.symlink += 1,
            DiagnosticCategory::OutOfScope => self.proposal.counts.out_of_scope += 1,
        }
        self.proposal.diagnostics.push(DiscoveryDiagnostic {
            category,
            path: path.to_string_lossy().into_owned(),
            message: message.into(),
        });
    }
}

fn validate_absolute(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute local path"));
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(format!("{label} must not contain parent traversal"));
    }
    Ok(())
}

fn resolve_grants(granted_roots: Vec<PathBuf>) -> Result<Vec<GrantedRoot>, String> {
    if granted_roots.is_empty() {
        return Err("At least one active granted root is required".to_owned());
    }

    let mut grants = Vec::new();
    for original in granted_roots {
        validate_absolute(&original, "Granted root")?;
        let metadata = fs::symlink_metadata(&original)
            .map_err(|error| format!("Granted root is unavailable: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("Granted root must not be a symbolic link".to_owned());
        }
        if !metadata.is_dir() && !metadata.is_file() {
            return Err("Granted root must be a file or directory".to_owned());
        }
        let canonical = fs::canonicalize(&original)
            .map_err(|error| format!("Granted root cannot be resolved: {error}"))?;
        grants.push(GrantedRoot {
            original,
            canonical,
        });
    }
    grants.sort_by(|left, right| left.canonical.cmp(&right.canonical));
    grants.dedup_by(|left, right| left.canonical == right.canonical);
    Ok(grants)
}

fn propose_local_drop_with_probe<F>(
    dropped_paths: Vec<PathBuf>,
    granted_roots: Vec<PathBuf>,
    max_items: usize,
    readability_probe: F,
) -> Result<DiscoveryProposal, String>
where
    F: Fn(&Path) -> io::Result<()>,
{
    if dropped_paths.is_empty() {
        return Err("At least one dropped local path is required".to_owned());
    }
    if max_items == 0 {
        return Err("Discovery item limit must be positive".to_owned());
    }
    for path in &dropped_paths {
        validate_absolute(path, "Dropped path")?;
    }
    let grants = resolve_grants(granted_roots)?;
    let mut paths = dropped_paths;
    paths.sort();
    paths.dedup();

    let mut state = DiscoveryState {
        grants: &grants,
        max_items,
        readability_probe: &readability_probe,
        visited_count: 0,
        limit_reported: false,
        seen_paths: BTreeSet::new(),
        seen_directories: BTreeSet::new(),
        seen_files: BTreeSet::new(),
        proposal: DiscoveryProposal::default(),
    };
    for path in paths {
        state.inspect(path);
    }
    state.proposal.items.sort_by(|left, right| left.path.cmp(&right.path));
    state.proposal.diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.category.cmp(&right.category))
    });
    Ok(state.proposal)
}

#[tauri::command]
pub fn propose_local_drop(
    dropped_paths: Vec<String>,
    granted_roots: Vec<String>,
) -> Result<DiscoveryProposal, String> {
    propose_local_drop_with_probe(
        dropped_paths.into_iter().map(PathBuf::from).collect(),
        granted_roots.into_iter().map(PathBuf::from).collect(),
        MAX_DISCOVERY_ITEMS,
        |path| File::open(path).map(|_| ()),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        propose_local_drop_with_probe, DiagnosticCategory, DiscoveryProposal,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::io;
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
                "aiknowledgesort-discovery-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir(&root).expect("create generated temporary tree");
            Self { root }
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove generated temporary tree");
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SnapshotEntry {
        kind: &'static str,
        len: u64,
        readonly: bool,
        modified_nanos: u128,
        bytes: Vec<u8>,
    }

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, SnapshotEntry> {
        fn visit(
            root: &Path,
            path: &Path,
            entries: &mut BTreeMap<PathBuf, SnapshotEntry>,
        ) {
            let mut children = fs::read_dir(path)
                .expect("snapshot directory is readable")
                .map(|entry| entry.expect("snapshot entry").path())
                .collect::<Vec<_>>();
            children.sort();

            for child in children {
                let metadata = fs::symlink_metadata(&child).expect("snapshot metadata");
                let file_type = metadata.file_type();
                let kind = if file_type.is_symlink() {
                    "symlink"
                } else if file_type.is_dir() {
                    "directory"
                } else {
                    "file"
                };
                let bytes = if file_type.is_file() {
                    fs::read(&child).expect("snapshot file bytes")
                } else {
                    Vec::new()
                };
                let modified_nanos = metadata
                    .modified()
                    .expect("snapshot modification time")
                    .duration_since(UNIX_EPOCH)
                    .expect("modification after epoch")
                    .as_nanos();
                entries.insert(
                    child.strip_prefix(root).expect("path below root").to_path_buf(),
                    SnapshotEntry {
                        kind,
                        len: metadata.len(),
                        readonly: metadata.permissions().readonly(),
                        modified_nanos,
                        bytes,
                    },
                );
                if file_type.is_dir() {
                    visit(root, &child, entries);
                }
            }
        }

        let mut entries = BTreeMap::new();
        visit(root, root, &mut entries);
        entries
    }

    fn always_readable(_: &Path) -> io::Result<()> {
        Ok(())
    }

    #[test]
    fn discovers_scoped_files_once_and_reports_boundaries_without_mutation() {
        let tree = TempTree::new();
        let granted = tree.path("granted");
        let dropped_directory = granted.join("drop/a");
        let subdirectory = dropped_directory.join("sub");
        let one = dropped_directory.join("one.txt");
        let two = subdirectory.join("two.txt");
        let unreadable = dropped_directory.join("unreadable.txt");
        let missing = dropped_directory.join("missing.txt");
        let outside = tree.path("outside.txt");

        fs::create_dir_all(&subdirectory).expect("create generated directories");
        fs::write(&one, b"one\n").expect("write generated file");
        fs::write(&two, b"two\n").expect("write generated file");
        fs::write(&unreadable, b"blocked\n").expect("write generated file");
        fs::write(&outside, b"outside\n").expect("write generated file");

        #[cfg(unix)]
        let link = {
            let link = dropped_directory.join("one-link.txt");
            std::os::unix::fs::symlink(&one, &link).expect("create generated symlink");
            link
        };

        let before = snapshot(&tree.root);
        let mut dropped_paths = vec![
            dropped_directory.clone(),
            one.clone(),
            subdirectory.clone(),
            outside.clone(),
            missing.clone(),
        ];
        #[cfg(unix)]
        dropped_paths.push(link);

        let proposal = propose_local_drop_with_probe(
            dropped_paths,
            vec![granted],
            100,
            |path| {
                if path == unreadable {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "deterministic unreadable boundary",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .expect("valid discovery proposal");

        let expected_paths = vec![
            fs::canonicalize(&one)
                .expect("canonical one")
                .to_string_lossy()
                .into_owned(),
            fs::canonicalize(&two)
                .expect("canonical two")
                .to_string_lossy()
                .into_owned(),
        ];
        assert_eq!(
            proposal
                .items
                .iter()
                .map(|item| item.path.clone())
                .collect::<Vec<_>>(),
            expected_paths
        );
        assert_eq!(proposal.counts.included, 2);
        assert_eq!(proposal.counts.excluded, 1);
        assert_eq!(proposal.counts.unreadable, 1);
        assert_eq!(proposal.counts.out_of_scope, 1);
        #[cfg(unix)]
        assert_eq!(proposal.counts.symlink, 1);
        #[cfg(not(unix))]
        assert_eq!(proposal.counts.symlink, 0);
        assert!(proposal.diagnostics.iter().any(|diagnostic| {
            diagnostic.category == DiagnosticCategory::Unreadable
                && diagnostic.path == unreadable.to_string_lossy()
        }));
        assert!(proposal.diagnostics.iter().any(|diagnostic| {
            diagnostic.category == DiagnosticCategory::OutOfScope
                && diagnostic.path == outside.to_string_lossy()
        }));
        assert_eq!(snapshot(&tree.root), before);
    }

    #[test]
    fn rejects_empty_grants_and_relative_inputs() {
        let tree = TempTree::new();
        let file = tree.path("one.txt");
        fs::write(&file, b"one\n").expect("write generated file");

        assert!(propose_local_drop_with_probe(
            vec![file.clone()],
            Vec::new(),
            100,
            always_readable,
        )
        .is_err());
        assert!(propose_local_drop_with_probe(
            Vec::new(),
            vec![tree.root.clone()],
            100,
            always_readable,
        )
        .is_err());
        assert!(propose_local_drop_with_probe(
            vec![PathBuf::from("relative.txt")],
            vec![tree.root.clone()],
            100,
            always_readable,
        )
        .is_err());
        assert!(propose_local_drop_with_probe(
            vec![file],
            vec![PathBuf::from("relative-grant")],
            100,
            always_readable,
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_external_symlink_route_into_a_granted_root() {
        let tree = TempTree::new();
        let granted = tree.path("granted");
        let file = granted.join("one.txt");
        let alias = tree.path("grant-alias");
        fs::create_dir(&granted).expect("create generated grant");
        fs::write(&file, b"one\n").expect("write generated file");
        std::os::unix::fs::symlink(&granted, &alias).expect("create generated alias");

        let proposal = propose_local_drop_with_probe(
            vec![alias.join("one.txt")],
            vec![granted],
            100,
            always_readable,
        )
        .expect("valid discovery proposal");

        assert!(proposal.items.is_empty());
        assert_eq!(proposal.counts.symlink, 1);
    }

    #[test]
    fn bounds_traversal_and_keeps_results_stably_sorted() {
        let tree = TempTree::new();
        let granted = tree.path("granted");
        fs::create_dir(&granted).expect("create generated grant");
        for name in ["c.txt", "a.txt", "b.txt"] {
            fs::write(granted.join(name), name.as_bytes()).expect("write generated file");
        }

        let proposal = propose_local_drop_with_probe(
            vec![granted.clone()],
            vec![granted],
            3,
            always_readable,
        )
        .expect("bounded discovery proposal");

        assert_eq!(proposal.counts.included, 2);
        assert_eq!(
            proposal
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.txt", "b.txt"]
        );
        assert!(proposal
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.category == DiagnosticCategory::TraversalLimit));
    }

    #[test]
    fn serializes_frontend_contract_in_camel_case() {
        let proposal = DiscoveryProposal::default();
        let value = serde_json::to_value(proposal).expect("serialize proposal");

        assert!(value["counts"].get("outOfScope").is_some());
        assert!(value["counts"].get("out_of_scope").is_none());
    }
}

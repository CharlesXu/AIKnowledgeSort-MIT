use super::grant::{DropGrantRegistry, RegistryLimits};
use super::walker::{
    discover_grant_with_hooks, discover_grant_with_hooks_and_deadline, discover_grant_with_limit,
};
use super::{DiagnosticCategory, DiscoveryProposal, DropWorkLimiter};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::sync::mpsc;
#[cfg(unix)]
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let root = root
            .canonicalize()
            .expect("canonical generated temporary tree root");
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
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, SnapshotEntry>) {
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
            entries.insert(
                child
                    .strip_prefix(root)
                    .expect("path below root")
                    .to_path_buf(),
                SnapshotEntry {
                    kind,
                    len: metadata.len(),
                    readonly: metadata.permissions().readonly(),
                    modified_nanos: metadata
                        .modified()
                        .expect("snapshot modification time")
                        .duration_since(UNIX_EPOCH)
                        .expect("modification after epoch")
                        .as_nanos(),
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

fn issue_and_consume(
    registry: &DropGrantRegistry,
    paths: Vec<PathBuf>,
    now: Instant,
) -> super::grant::DropGrant {
    let issued = registry
        .issue_at(paths, now)
        .expect("issue trusted test grant");
    registry
        .consume_at(&issued.grant_id, now)
        .expect("consume trusted test grant")
}

#[cfg(unix)]
fn create_fifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "mkfifo must create the generated FIFO");
}

#[cfg(unix)]
fn finish_without_blocking(
    operation: impl FnOnce() -> DiscoveryProposal + Send + 'static,
) -> DiscoveryProposal {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ = sender.send(operation());
    });
    receiver
        .recv_timeout(Duration::from_millis(500))
        .expect("special filesystem item handling must not block")
}

#[test]
fn denies_spoofed_reused_and_expired_grant_ids() {
    let tree = TempTree::new();
    let file = tree.path("one.txt");
    fs::write(&file, b"one\n").expect("write generated file");
    let now = Instant::now();
    let registry = DropGrantRegistry::new(RegistryLimits {
        max_grants: 4,
        max_paths: 4,
        max_path_bytes: 4_096,
        ttl: Duration::from_secs(5),
    });

    assert!(registry.consume_at("spoofed", now).is_err());
    let issued = registry
        .issue_at(vec![file.clone()], now)
        .expect("issue trusted test grant");
    assert!(registry.consume_at(&issued.grant_id, now).is_ok());
    assert!(registry.consume_at(&issued.grant_id, now).is_err());

    let expired = registry
        .issue_at(vec![file], now)
        .expect("issue expiring test grant");
    assert!(registry
        .consume_at(&expired.grant_id, now + Duration::from_secs(6))
        .is_err());
}

#[test]
fn bounds_grant_count_dropped_path_count_and_path_length() {
    let tree = TempTree::new();
    let one = tree.path("one.txt");
    let two = tree.path("two.txt");
    fs::write(&one, b"one\n").expect("write generated file");
    fs::write(&two, b"two\n").expect("write generated file");
    let now = Instant::now();
    let registry = DropGrantRegistry::new(RegistryLimits {
        max_grants: 1,
        max_paths: 1,
        max_path_bytes: one.to_string_lossy().len(),
        ttl: Duration::from_secs(5),
    });

    assert!(registry
        .issue_at(vec![one.clone(), two.clone()], now)
        .is_err());
    assert!(registry
        .issue_at(vec![PathBuf::from(format!("{}x", one.display()))], now)
        .is_err());
    registry
        .issue_at(vec![one], now)
        .expect("issue bounded grant");
    assert!(registry.issue_at(vec![two], now).is_err());
}

#[test]
fn expired_worker_cannot_insert_a_hidden_grant() {
    let tree = TempTree::new();
    let delayed = tree.path("delayed.txt");
    let valid = tree.path("valid.txt");
    fs::write(&delayed, b"delayed\n").expect("write delayed file");
    fs::write(&valid, b"valid\n").expect("write valid file");
    let registry = DropGrantRegistry::new(RegistryLimits {
        max_grants: 1,
        max_paths: 2,
        max_path_bytes: 4_096,
        ttl: Duration::from_secs(5),
    });
    let deadline = Instant::now() + Duration::from_millis(10);

    let expired = registry.issue_with_deadline_and_hook(vec![delayed], deadline, || {
        std::thread::sleep(Duration::from_millis(30))
    });

    assert!(expired.is_err());
    registry
        .issue_at(vec![valid], Instant::now())
        .expect("expired worker must leave registry capacity available");
}

#[test]
fn bounds_blocking_filesystem_work_and_releases_capacity() {
    let limiter = DropWorkLimiter::new(1);
    let permit = limiter.try_acquire().expect("first work permit");

    assert!(limiter.try_acquire().is_err());
    drop(permit);
    assert!(limiter.try_acquire().is_ok());
}

#[test]
fn discovers_overlapping_roots_and_boundaries_without_mutation() {
    let tree = TempTree::new();
    let directory = tree.path("drop/a");
    let subdirectory = directory.join("sub");
    let one = directory.join("one.txt");
    let two = subdirectory.join("two.txt");
    let unreadable = directory.join("unreadable.txt");
    let missing = directory.join("missing.txt");
    fs::create_dir_all(&subdirectory).expect("create generated directories");
    fs::write(&one, b"one\n").expect("write generated file");
    fs::write(&two, b"two\n").expect("write generated file");
    fs::write(&unreadable, b"blocked\n").expect("write generated file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&one, directory.join("one-link.txt"))
        .expect("create generated symlink");

    let before = snapshot(&tree.root);
    let now = Instant::now();
    let registry = DropGrantRegistry::default();
    let grant = issue_and_consume(
        &registry,
        vec![directory.clone(), one.clone(), subdirectory, missing],
        now,
    );
    let proposal = discover_grant_with_hooks(
        grant,
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
        |_| {},
    )
    .expect("valid capability discovery");

    assert_eq!(
        proposal
            .items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["one.txt", "two.txt"]
    );
    assert_eq!(proposal.counts.included, 2);
    assert_eq!(proposal.counts.excluded, 1);
    assert_eq!(proposal.counts.unreadable, 1);
    #[cfg(unix)]
    assert_eq!(proposal.counts.symlink, 1);
    assert_eq!(proposal.counts.out_of_scope, 0);
    assert_eq!(snapshot(&tree.root), before);
}

#[test]
fn excludes_an_oversized_directory_as_one_bounded_diagnostic() {
    let tree = TempTree::new();
    let directory = tree.path("oversized");
    fs::create_dir(&directory).expect("create generated directory");
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(directory.join(name), name.as_bytes()).expect("write generated file");
    }
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![directory], now);
    let proposal = discover_grant_with_limit(grant, 3).expect("bounded discovery proposal");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.excluded, 1);
    assert_eq!(proposal.diagnostics.len(), 1);
    assert_eq!(
        proposal.diagnostics[0].category,
        DiagnosticCategory::TraversalLimit
    );
}

#[test]
fn traversal_deadline_returns_one_visible_bounded_failure() {
    let tree = TempTree::new();
    let directory = tree.path("deadline");
    fs::create_dir(&directory).expect("create generated directory");
    fs::write(directory.join("one.txt"), b"one\n").expect("write generated file");
    let registry = DropGrantRegistry::default();
    let grant = issue_and_consume(&registry, vec![directory], Instant::now());

    let proposal = discover_grant_with_hooks_and_deadline(
        grant,
        10,
        Instant::now() + Duration::from_millis(10),
        |_| Ok(()),
        |_| std::thread::sleep(Duration::from_millis(30)),
    )
    .expect("deadline produces a proposal");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.diagnostics.len(), 1);
    assert_eq!(
        proposal.diagnostics[0].category,
        DiagnosticCategory::TraversalLimit
    );
    assert!(proposal.diagnostics[0].message.contains("deadline"));
}

#[cfg(unix)]
#[test]
fn nofollow_open_rejects_a_file_swapped_to_a_symlink_during_traversal() {
    let tree = TempTree::new();
    let directory = tree.path("drop");
    let victim = directory.join("victim.txt");
    let outside = tree.path("outside.txt");
    fs::create_dir(&directory).expect("create generated directory");
    fs::write(&victim, b"inside\n").expect("write generated file");
    fs::write(&outside, b"outside\n").expect("write generated file");
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![directory], now);
    let mut swapped = false;

    let proposal = discover_grant_with_hooks(
        grant,
        10,
        |_| Ok(()),
        |path| {
            if path == victim && !swapped {
                swapped = true;
                fs::remove_file(&victim).expect("remove generated victim");
                std::os::unix::fs::symlink(&outside, &victim)
                    .expect("replace victim with generated symlink");
            }
        },
    )
    .expect("safe discovery after path swap");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.unreadable + proposal.counts.symlink, 1);
}

#[cfg(unix)]
#[test]
fn file_root_uses_the_issued_handle_after_its_name_is_replaced() {
    let tree = TempTree::new();
    let file = tree.path("root.txt");
    let moved = tree.path("moved.txt");
    let outside = tree.path("outside.txt");
    fs::write(&file, b"safe\n").expect("write generated file");
    fs::write(&outside, b"outside content\n").expect("write generated file");
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![file.clone()], now);
    fs::rename(&file, &moved).expect("move generated file");
    std::os::unix::fs::symlink(&outside, &file).expect("replace name with symlink");

    let proposal = discover_grant_with_limit(grant, 10).expect("discover issued file handle");

    assert_eq!(proposal.items.len(), 1);
    assert_eq!(proposal.items[0].path, file.to_string_lossy());
    assert_eq!(proposal.items[0].byte_size, 5);
}

#[cfg(unix)]
#[test]
fn descendant_below_a_symlink_parent_is_rejected() {
    let tree = TempTree::new();
    let real_directory = tree.path("real");
    let alias = tree.path("alias");
    let file = real_directory.join("inside.txt");
    fs::create_dir(&real_directory).expect("create generated real directory");
    fs::write(&file, b"inside\n").expect("write generated file");
    std::os::unix::fs::symlink(&real_directory, &alias).expect("create generated parent symlink");
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![alias.join("inside.txt")], now);

    let proposal = discover_grant_with_limit(grant, 10).expect("discover descendant safely");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.symlink, 1);
}

#[cfg(unix)]
#[test]
fn fifo_drop_root_is_excluded_without_blocking() {
    let tree = TempTree::new();
    let fifo = tree.path("root.fifo");
    create_fifo(&fifo);

    let proposal = finish_without_blocking(move || {
        let registry = DropGrantRegistry::default();
        let grant = issue_and_consume(&registry, vec![fifo], Instant::now());
        discover_grant_with_limit(grant, 10).expect("discover FIFO root safely")
    });

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.excluded, 1);
}

#[cfg(unix)]
#[test]
fn fifo_directory_child_is_excluded_without_blocking() {
    let tree = TempTree::new();
    let directory = tree.path("drop");
    fs::create_dir(&directory).expect("create generated directory");
    create_fifo(&directory.join("child.fifo"));

    let proposal = finish_without_blocking(move || {
        let registry = DropGrantRegistry::default();
        let grant = issue_and_consume(&registry, vec![directory], Instant::now());
        discover_grant_with_limit(grant, 10).expect("discover FIFO child safely")
    });

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.excluded, 1);
}

#[cfg(windows)]
#[test]
fn windows_symlink_reparse_root_is_rejected_when_creation_is_permitted() {
    let tree = TempTree::new();
    let target = tree.path("target.txt");
    let link = tree.path("link.txt");
    fs::write(&target, b"target\n").expect("write generated file");
    if std::os::windows::fs::symlink_file(&target, &link).is_err() {
        return;
    }
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![link], now);
    let proposal = discover_grant_with_limit(grant, 10).expect("discover link root");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.symlink, 1);
}

#[cfg(windows)]
#[test]
fn windows_symlink_parent_is_rejected_when_creation_is_permitted() {
    let tree = TempTree::new();
    let real_directory = tree.path("real");
    let alias = tree.path("alias");
    fs::create_dir(&real_directory).expect("create generated real directory");
    fs::write(real_directory.join("inside.txt"), b"inside\n").expect("write generated file");
    if std::os::windows::fs::symlink_dir(&real_directory, &alias).is_err() {
        return;
    }
    let registry = DropGrantRegistry::default();
    let now = Instant::now();
    let grant = issue_and_consume(&registry, vec![alias.join("inside.txt")], now);
    let proposal = discover_grant_with_limit(grant, 10).expect("discover descendant safely");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.symlink, 1);
}

#[cfg(windows)]
#[test]
fn windows_device_namespace_root_is_rejected_before_open() {
    let registry = DropGrantRegistry::default();
    let grant = issue_and_consume(&registry, vec![PathBuf::from(r"\\.\NUL")], Instant::now());
    let proposal = discover_grant_with_limit(grant, 10).expect("reject device namespace");

    assert!(proposal.items.is_empty());
    assert_eq!(proposal.counts.excluded, 1);
}

#[test]
fn serializes_frontend_contract_in_camel_case() {
    let value = serde_json::to_value(DiscoveryProposal::default()).expect("serialize proposal");

    assert!(value["counts"].get("outOfScope").is_some());
    assert!(value["counts"].get("out_of_scope").is_none());
}

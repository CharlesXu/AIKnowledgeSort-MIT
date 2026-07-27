use super::grant::{
    file_type_is_link, path_display_len, read_only_nofollow_options, CapabilityRoot, DropGrant,
};
use super::{
    DiagnosticCategory, DiscoveredItem, DiscoveryDiagnostic, DiscoveryProposal, MAX_DIAGNOSTICS,
    MAX_DIRECTORY_DEPTH, MAX_DISCOVERY_ITEMS, MAX_PATH_BYTES,
};
use crate::identity::ContentIdentity;
use cap_std::fs::{Dir, DirEntry, File};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct CapabilityDiscovery<'a, F, H>
where
    F: FnMut(&Path) -> io::Result<()>,
    H: FnMut(&Path),
{
    max_items: usize,
    visited_items: usize,
    seen_paths: BTreeSet<PathBuf>,
    proposal: DiscoveryProposal,
    readability_probe: &'a mut F,
    before_open: &'a mut H,
    deadline: Instant,
    deadline_reported: bool,
}

impl<F, H> CapabilityDiscovery<'_, F, H>
where
    F: FnMut(&Path) -> io::Result<()>,
    H: FnMut(&Path),
{
    fn inspect_root(&mut self, root: CapabilityRoot) {
        let display_path = match &root {
            CapabilityRoot::Directory { display_path, .. }
            | CapabilityRoot::File { display_path, .. }
            | CapabilityRoot::Diagnostic { display_path, .. } => display_path.clone(),
        };
        if self.stop_for_deadline(&display_path) {
            return;
        }
        match root {
            CapabilityRoot::Directory {
                display_path,
                directory,
            } => self.inspect_directory(display_path, directory, 0),
            CapabilityRoot::File { display_path, file } => {
                if self.visited_items >= self.max_items {
                    self.diagnostic(
                        DiagnosticCategory::TraversalLimit,
                        &display_path,
                        "Discovery item limit was reached",
                    );
                    return;
                }
                self.visited_items += 1;
                self.include_open_file(display_path, file);
            }
            CapabilityRoot::Diagnostic {
                display_path,
                category,
                message,
            } => self.diagnostic(category, &display_path, message),
        }
    }

    fn inspect_directory(&mut self, display_path: PathBuf, directory: Dir, depth: usize) {
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if self.seen_paths.contains(&display_path) {
            return;
        }
        if depth > MAX_DIRECTORY_DEPTH {
            self.diagnostic(
                DiagnosticCategory::TraversalLimit,
                &display_path,
                "Directory depth limit was reached",
            );
            return;
        }
        let remaining = self.max_items.saturating_sub(self.visited_items);
        if remaining == 0 {
            self.diagnostic(
                DiagnosticCategory::TraversalLimit,
                &display_path,
                "Directory excluded because no traversal budget remains",
            );
            return;
        }
        let iterator = match directory.entries() {
            Ok(iterator) => iterator,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Directory is unreadable: {error}"),
                );
                return;
            }
        };
        let mut entries = Vec::with_capacity(remaining.saturating_add(1));
        let iterator = iterator.take(remaining.saturating_add(1));
        for entry in iterator {
            if self.stop_for_deadline(&display_path) {
                return;
            }
            entries.push(entry);
        }
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if entries.len() > remaining {
            self.diagnostic(
                DiagnosticCategory::TraversalLimit,
                &display_path,
                "Directory subtree exceeds the remaining traversal budget",
            );
            return;
        }

        self.seen_paths.insert(display_path.clone());
        self.visited_items += entries.len();
        entries.sort_by(|left, right| match (left, right) {
            (Ok(left), Ok(right)) => left.file_name().cmp(&right.file_name()),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => std::cmp::Ordering::Equal,
        });
        for entry in entries {
            match entry {
                Ok(entry) => self.inspect_entry(&display_path, entry, depth + 1),
                Err(error) => self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Directory entry is unreadable: {error}"),
                ),
            }
        }
    }

    fn inspect_entry(&mut self, parent_display: &Path, entry: DirEntry, depth: usize) {
        let display_path = parent_display.join(entry.file_name());
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if path_display_len(&display_path) > MAX_PATH_BYTES {
            self.diagnostic(
                DiagnosticCategory::Excluded,
                &display_path,
                "Discovered path exceeds the configured length limit",
            );
            return;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Entry type is unreadable: {error}"),
                );
                return;
            }
        };
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if file_type_is_link(&file_type) {
            self.diagnostic(
                DiagnosticCategory::Symlink,
                &display_path,
                "Links and Windows reparse-point links are excluded",
            );
            return;
        }
        if !file_type.is_file() && !file_type.is_dir() {
            self.diagnostic(
                DiagnosticCategory::Excluded,
                &display_path,
                "Entry is not a regular file or directory",
            );
            return;
        }

        (self.before_open)(&display_path);
        if self.stop_for_deadline(&display_path) {
            return;
        }
        let opened = match entry.open_with(&read_only_nofollow_options()) {
            Ok(opened) => opened,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Entry cannot be opened without links: {error}"),
                );
                return;
            }
        };
        if self.stop_for_deadline(&display_path) {
            return;
        }
        self.inspect_opened_entry(display_path, opened, depth);
    }

    fn inspect_opened_entry(&mut self, display_path: PathBuf, opened: File, depth: usize) {
        if self.stop_for_deadline(&display_path) {
            return;
        }
        let metadata = match opened.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Opened entry metadata is unreadable: {error}"),
                );
                return;
            }
        };
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if file_type_is_link(&metadata.file_type()) {
            self.diagnostic(
                DiagnosticCategory::Symlink,
                &display_path,
                "Opened entry is a link or reparse point",
            );
        } else if metadata.is_dir() {
            match Dir::reopen_dir(&opened) {
                Ok(directory) => self.inspect_directory(display_path, directory, depth),
                Err(error) => self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("Directory capability cannot be reopened: {error}"),
                ),
            }
        } else if metadata.is_file() {
            self.include_open_file(display_path, opened);
        } else {
            self.diagnostic(
                DiagnosticCategory::Excluded,
                &display_path,
                "Unsupported filesystem item",
            );
        }
    }

    fn include_open_file(&mut self, display_path: PathBuf, mut file: File) {
        if self.stop_for_deadline(&display_path) {
            return;
        }
        if !self.seen_paths.insert(display_path.clone()) {
            return;
        }
        if let Err(error) = (self.readability_probe)(&display_path) {
            self.diagnostic(
                DiagnosticCategory::Unreadable,
                &display_path,
                format!("File is unreadable: {error}"),
            );
            return;
        }
        if self.stop_for_deadline(&display_path) {
            return;
        }
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("File handle metadata is unreadable: {error}"),
                );
                return;
            }
        };
        if self.stop_for_deadline(&display_path) {
            return;
        }
        let name = display_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| display_path.to_string_lossy().into_owned());
        let identity = match ContentIdentity::from_reader(&mut file) {
            Ok(identity) => identity,
            Err(error) => {
                self.diagnostic(
                    DiagnosticCategory::Unreadable,
                    &display_path,
                    format!("File content cannot be hashed: {error}"),
                );
                return;
            }
        };
        self.proposal.items.push(DiscoveredItem {
            item_id: uuid::Uuid::new_v4().simple().to_string(),
            path: display_path.to_string_lossy().into_owned(),
            name,
            byte_size: metadata.len(),
            identity,
        });
        self.proposal.counts.included += 1;
    }

    fn stop_for_deadline(&mut self, path: &Path) -> bool {
        if Instant::now() < self.deadline {
            return false;
        }
        if !self.deadline_reported {
            self.deadline_reported = true;
            self.diagnostic(
                DiagnosticCategory::TraversalLimit,
                path,
                "Filesystem discovery deadline exceeded",
            );
        }
        true
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
        }
        if self.proposal.diagnostics.len() >= MAX_DIAGNOSTICS {
            return;
        }
        self.proposal.diagnostics.push(DiscoveryDiagnostic {
            category,
            path: bounded_text(&path.to_string_lossy(), MAX_PATH_BYTES),
            message: bounded_text(&message.into(), 512),
        });
    }
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
pub(super) fn discover_grant_with_hooks<F, H>(
    grant: DropGrant,
    max_items: usize,
    readability_probe: F,
    before_open: H,
) -> Result<DiscoveryProposal, String>
where
    F: FnMut(&Path) -> io::Result<()>,
    H: FnMut(&Path),
{
    discover_grant_with_hooks_and_deadline(
        grant,
        max_items,
        Instant::now() + super::DROP_WORK_TIMEOUT,
        readability_probe,
        before_open,
    )
}

pub(super) fn discover_grant_with_hooks_and_deadline<F, H>(
    grant: DropGrant,
    max_items: usize,
    deadline: Instant,
    mut readability_probe: F,
    mut before_open: H,
) -> Result<DiscoveryProposal, String>
where
    F: FnMut(&Path) -> io::Result<()>,
    H: FnMut(&Path),
{
    if max_items == 0 || max_items > MAX_DISCOVERY_ITEMS {
        return Err("Discovery item limit is invalid".to_owned());
    }
    let mut discovery = CapabilityDiscovery {
        max_items,
        visited_items: 0,
        seen_paths: BTreeSet::new(),
        proposal: DiscoveryProposal::default(),
        readability_probe: &mut readability_probe,
        before_open: &mut before_open,
        deadline,
        deadline_reported: false,
    };
    for root in grant.roots {
        discovery.inspect_root(root);
    }
    discovery
        .proposal
        .items
        .sort_by(|left, right| left.path.cmp(&right.path));
    discovery.proposal.diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.category.cmp(&right.category))
    });
    Ok(discovery.proposal)
}

#[cfg(test)]
pub(super) fn discover_grant_with_limit(
    grant: DropGrant,
    max_items: usize,
) -> Result<DiscoveryProposal, String> {
    discover_grant_with_hooks(grant, max_items, |_| Ok(()), |_| {})
}

pub(super) fn discover_grant_with_deadline(
    grant: DropGrant,
    max_items: usize,
    deadline: Instant,
) -> Result<DiscoveryProposal, String> {
    discover_grant_with_hooks_and_deadline(grant, max_items, deadline, |_| Ok(()), |_| {})
}

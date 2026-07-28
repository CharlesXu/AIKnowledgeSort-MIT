use super::schema::{
    issue_token, tool_catalog, validate_safe_id, AgentAccessState, AgentGrantStatus,
    AgentGrantSummary, AgentScopeSummary, CreateAgentGrantRequest, IssuedAgentGrant,
    NativeScopeSelection, MAX_SCOPES, TOOL_CATALOG_VERSION,
};
use super::store::{AgentAccessStore, AgentGrantRecord, PersistedAgentAccess};
use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use cap_std::fs::Dir;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_PENDING_SELECTIONS: usize = 16;
const SELECTION_TTL: Duration = Duration::from_secs(5 * 60);

struct SelectedRoot {
    summary: AgentScopeSummary,
    directory: Dir,
}

struct PendingSelection {
    expires_at: SystemTime,
    roots: Vec<SelectedRoot>,
}

struct ActiveGrant {
    roots: HashMap<String, SelectedRoot>,
}

struct State {
    config_root: Option<PathBuf>,
    persisted: PersistedAgentAccess,
    pending_selections: HashMap<String, PendingSelection>,
    active_grants: HashMap<String, ActiveGrant>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config_root: None,
            persisted: PersistedAgentAccess::default(),
            pending_selections: HashMap::new(),
            active_grants: HashMap::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct AgentAccessAuthority {
    state: Arc<Mutex<State>>,
}

impl AgentAccessAuthority {
    pub fn select_paths(
        &self,
        paths: Vec<PathBuf>,
        now: SystemTime,
    ) -> Result<NativeScopeSelection, String> {
        if paths.is_empty() || paths.len() > MAX_SCOPES {
            return Err("Select between 1 and 16 Agent scope directories".to_owned());
        }
        let mut paths = paths;
        paths.sort();
        paths.dedup();
        let mut roots = Vec::with_capacity(paths.len());
        let mut displays = HashSet::with_capacity(paths.len());
        for path in paths {
            let (display_path, directory) = match open_trusted_drop_root(path) {
                CapabilityRoot::Directory {
                    display_path,
                    directory,
                } => (display_path, directory),
                CapabilityRoot::File { .. } => {
                    return Err("Agent scopes must be directories".to_owned())
                }
                CapabilityRoot::Diagnostic { message, .. } => return Err(message),
            };
            let display = display_path.to_string_lossy().into_owned();
            if displays.insert(display.clone()) {
                roots.push(SelectedRoot {
                    summary: AgentScopeSummary {
                        scope_id: Uuid::new_v4().simple().to_string(),
                        display_path: display,
                    },
                    directory,
                });
            }
        }
        if roots.is_empty() {
            return Err("No trusted Agent scope directories were selected".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Agent access authority is unavailable".to_owned())?;
        prune_pending(&mut state, now);
        if state.pending_selections.len() >= MAX_PENDING_SELECTIONS {
            return Err("Too many pending Agent scope selections".to_owned());
        }
        let selection_id = Uuid::new_v4().simple().to_string();
        let scopes = roots.iter().map(|root| root.summary.clone()).collect();
        state.pending_selections.insert(
            selection_id.clone(),
            PendingSelection {
                expires_at: now + SELECTION_TTL,
                roots,
            },
        );
        Ok(NativeScopeSelection {
            selection_id,
            scopes,
        })
    }

    pub fn create_grant(
        &self,
        config_root: &Path,
        request: CreateAgentGrantRequest,
        now: SystemTime,
    ) -> Result<IssuedAgentGrant, String> {
        request.validate()?;
        let now_ms = unix_ms(now)?;
        let expires_at_unix_ms = now_ms
            .checked_add(
                request
                    .expires_in_seconds
                    .checked_mul(1000)
                    .ok_or_else(|| "Agent grant expiry overflows".to_owned())?,
            )
            .ok_or_else(|| "Agent grant expiry overflows".to_owned())?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Agent access authority is unavailable".to_owned())?;
        ensure_loaded(&mut state, config_root)?;
        prune_pending(&mut state, now);
        let selection = state
            .pending_selections
            .remove(&request.selection_id)
            .ok_or_else(|| {
                "Unknown, expired, or already consumed Agent scope selection".to_owned()
            })?;
        if selection.expires_at <= now {
            return Err("Agent scope selection has expired".to_owned());
        }
        if state.persisted.grants.len() >= 32 {
            return Err("At most 32 Agent grants are allowed".to_owned());
        }
        let grant_id = Uuid::new_v4().simple().to_string();
        let (token, grant_token_sha256) = issue_token()?;
        let record = AgentGrantRecord {
            grant_id: grant_id.clone(),
            agent_id: request.agent_id,
            label: request.label,
            tool_ids: request.tool_ids,
            scopes: selection
                .roots
                .iter()
                .map(|root| root.summary.clone())
                .collect(),
            created_at_unix_ms: now_ms,
            expires_at_unix_ms,
            revoked_at_unix_ms: None,
            limits: request.limits,
            grant_token_sha256,
        };
        let mut next = state.persisted.clone();
        next.next_audit_sequence = next
            .next_audit_sequence
            .checked_add(1)
            .ok_or_else(|| "Agent audit sequence is exhausted".to_owned())?;
        next.grants.push(record.clone());
        next.grants
            .sort_by(|left, right| left.grant_id.cmp(&right.grant_id));
        AgentAccessStore::new(config_root.to_path_buf()).write_change(
            &next,
            "grant.created",
            &grant_id,
            now_ms,
        )?;
        let roots = selection
            .roots
            .into_iter()
            .map(|root| (root.summary.scope_id.clone(), root))
            .collect();
        state.persisted = next;
        state.active_grants.insert(grant_id, ActiveGrant { roots });
        Ok(IssuedAgentGrant {
            grant: summary(&record, true, now_ms),
            grant_token: token.into_string(),
        })
    }

    pub fn inspect(&self, config_root: &Path, now: SystemTime) -> Result<AgentAccessState, String> {
        let now_ms = unix_ms(now)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Agent access authority is unavailable".to_owned())?;
        ensure_loaded(&mut state, config_root)?;
        Ok(state_summary(&state, now_ms))
    }

    pub fn revoke(
        &self,
        config_root: &Path,
        grant_id: &str,
        now: SystemTime,
    ) -> Result<AgentAccessState, String> {
        validate_safe_id("Agent grant id", grant_id)?;
        let now_ms = unix_ms(now)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Agent access authority is unavailable".to_owned())?;
        ensure_loaded(&mut state, config_root)?;
        let Some(index) = state
            .persisted
            .grants
            .iter()
            .position(|grant| grant.grant_id == grant_id)
        else {
            return Err("Agent grant does not exist".to_owned());
        };
        if state.persisted.grants[index].revoked_at_unix_ms.is_none() {
            let mut next = state.persisted.clone();
            next.next_audit_sequence = next
                .next_audit_sequence
                .checked_add(1)
                .ok_or_else(|| "Agent audit sequence is exhausted".to_owned())?;
            next.grants[index].revoked_at_unix_ms = Some(now_ms);
            AgentAccessStore::new(config_root.to_path_buf()).write_change(
                &next,
                "grant.revoked",
                grant_id,
                now_ms,
            )?;
            state.persisted = next;
            state.active_grants.remove(grant_id);
        }
        Ok(state_summary(&state, now_ms))
    }
}

fn ensure_loaded(state: &mut State, config_root: &Path) -> Result<(), String> {
    if let Some(current) = &state.config_root {
        if current != config_root {
            return Err("Agent access configuration root changed during runtime".to_owned());
        }
        return Ok(());
    }
    state.persisted = AgentAccessStore::new(config_root.to_path_buf()).read()?;
    state.config_root = Some(config_root.to_path_buf());
    Ok(())
}

fn prune_pending(state: &mut State, now: SystemTime) {
    state
        .pending_selections
        .retain(|_, selection| selection.expires_at > now);
}

fn state_summary(state: &State, now_ms: u64) -> AgentAccessState {
    let mut grants = state
        .persisted
        .grants
        .iter()
        .map(|record| {
            summary(
                record,
                state.active_grants.contains_key(&record.grant_id),
                now_ms,
            )
        })
        .collect::<Vec<_>>();
    grants.sort_by(|left, right| left.grant_id.cmp(&right.grant_id));
    AgentAccessState {
        schema_version: state.persisted.schema_version,
        tool_catalog_version: TOOL_CATALOG_VERSION.to_owned(),
        tools: tool_catalog().to_vec(),
        grants,
    }
}

fn summary(record: &AgentGrantRecord, active: bool, now_ms: u64) -> AgentGrantSummary {
    let status = if record.revoked_at_unix_ms.is_some() {
        AgentGrantStatus::Revoked
    } else if record.expires_at_unix_ms <= now_ms {
        AgentGrantStatus::Expired
    } else if active {
        AgentGrantStatus::Active
    } else {
        AgentGrantStatus::Inactive
    };
    AgentGrantSummary {
        grant_id: record.grant_id.clone(),
        agent_id: record.agent_id.clone(),
        label: record.label.clone(),
        tool_ids: record.tool_ids.clone(),
        scopes: record.scopes.clone(),
        created_at_unix_ms: record.created_at_unix_ms,
        expires_at_unix_ms: record.expires_at_unix_ms,
        revoked_at_unix_ms: record.revoked_at_unix_ms,
        status,
        limits: record.limits.clone(),
    }
}

fn unix_ms(time: SystemTime) -> Result<u64, String> {
    let millis = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock is before the Unix epoch".to_owned())?
        .as_millis();
    u64::try_from(millis).map_err(|_| "System clock cannot be represented".to_owned())
}

#[cfg(test)]
mod tests {
    use super::AgentAccessAuthority;
    use crate::agent_access::schema::{
        AgentGrantStatus, AgentResourceLimits, CreateAgentGrantRequest,
    };
    use crate::agent_access::store::AgentAccessStore;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(label: &str) -> Self {
            let unique = format!(
                "aiks-agent-{label}-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir(&root).expect("create temp root");
            Self {
                root: root.canonicalize().expect("canonical temp root"),
            }
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(2_000_000_000)
    }

    fn request(selection_id: String) -> CreateAgentGrantRequest {
        CreateAgentGrantRequest {
            selection_id,
            agent_id: "codex-desktop".to_owned(),
            label: "Codex Desktop".to_owned(),
            tool_ids: vec!["capabilities.read".to_owned(), "graph.read".to_owned()],
            expires_in_seconds: 3_600,
            limits: AgentResourceLimits {
                max_requests_per_session: 1_000,
                max_request_bytes: 128 * 1024,
                max_response_bytes: 256 * 1024,
            },
        }
    }

    #[test]
    fn consumes_one_native_selection_into_one_active_grant() {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        let authority = AgentAccessAuthority::default();
        let selection = authority
            .select_paths(vec![scope.root.clone()], now())
            .expect("select scope");
        let issued = authority
            .create_grant(&config.root, request(selection.selection_id.clone()), now())
            .expect("create grant");

        assert_eq!(issued.grant.status, AgentGrantStatus::Active);
        assert_eq!(issued.grant.scopes, selection.scopes);
        assert_eq!(issued.grant_token.len(), 64);
        assert!(authority
            .create_grant(&config.root, request(selection.selection_id), now())
            .is_err());

        let persisted = fs::read_to_string(config.root.join("agent-access-v1.json"))
            .expect("read persisted grants");
        assert!(!persisted.contains(&issued.grant_token));
        assert!(persisted.contains("grantTokenSha256"));
        assert_eq!(
            authority.inspect(&config.root, now()).unwrap().grants.len(),
            1
        );
    }

    #[test]
    fn relaunch_preserves_metadata_but_marks_grants_inactive() {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        let authority = AgentAccessAuthority::default();
        let selection = authority
            .select_paths(vec![scope.root.clone()], now())
            .expect("select scope");
        authority
            .create_grant(&config.root, request(selection.selection_id), now())
            .expect("create grant");

        let reloaded = AgentAccessAuthority::default();
        let state = reloaded
            .inspect(&config.root, now())
            .expect("inspect reload");
        assert_eq!(state.grants[0].status, AgentGrantStatus::Inactive);
        assert_eq!(
            state.grants[0].scopes[0].display_path,
            scope.root.to_string_lossy()
        );
    }

    #[test]
    fn rejects_expired_reused_and_invalid_native_selections() {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        let authority = AgentAccessAuthority::default();
        let selection = authority
            .select_paths(vec![scope.root.clone(), scope.root.clone()], now())
            .expect("deduplicated selection");
        assert_eq!(selection.scopes.len(), 1);
        assert!(authority
            .create_grant(
                &config.root,
                request(selection.selection_id),
                now() + Duration::from_secs(301),
            )
            .is_err());

        let file = scope.root.join("file.txt");
        fs::write(&file, b"not a directory").expect("write file");
        assert!(authority.select_paths(vec![file], now()).is_err());
    }

    #[test]
    fn revoke_is_idempotent_and_prevents_active_state() {
        let config = TempTree::new("config");
        let scope = TempTree::new("scope");
        let authority = AgentAccessAuthority::default();
        let selection = authority
            .select_paths(vec![scope.root.clone()], now())
            .unwrap();
        let issued = authority
            .create_grant(&config.root, request(selection.selection_id), now())
            .unwrap();
        let first = authority
            .revoke(&config.root, &issued.grant.grant_id, now())
            .expect("revoke");
        let second = authority
            .revoke(&config.root, &issued.grant.grant_id, now())
            .expect("idempotent revoke");
        assert_eq!(first.grants[0].status, AgentGrantStatus::Revoked);
        assert_eq!(second.grants[0].status, AgentGrantStatus::Revoked);
    }

    #[test]
    fn config_and_audit_paths_reject_links_and_non_regular_entries() {
        let config = TempTree::new("config");
        fs::create_dir(config.root.join("agent-access-v1.json"))
            .expect("create invalid config path");
        assert!(AgentAccessStore::new(config.root.clone()).read().is_err());

        fs::remove_dir(config.root.join("agent-access-v1.json")).unwrap();
        fs::create_dir(config.root.join("agent-access-audit")).unwrap();
        fs::write(config.root.join("agent-access-audit/blocker"), b"x").unwrap();
        let authority = AgentAccessAuthority::default();
        assert!(authority.inspect(&config.root, now()).is_ok());
    }
}

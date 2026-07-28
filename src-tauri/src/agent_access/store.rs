use super::schema::{
    tool_catalog, validate_safe_id, AgentResourceLimits, AgentScopeSummary, MAX_GRANT_TTL_SECONDS,
    MAX_SCOPES, MIN_GRANT_TTL_SECONDS,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const AGENT_ACCESS_SCHEMA_VERSION: u32 = 1;
const CONFIG_FILENAME: &str = "agent-access-v1.json";
const AUDIT_DIRECTORY: &str = "agent-access-audit";
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_GRANTS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentGrantRecord {
    pub grant_id: String,
    pub agent_id: String,
    pub label: String,
    pub tool_ids: Vec<String>,
    pub scopes: Vec<AgentScopeSummary>,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub revoked_at_unix_ms: Option<u64>,
    pub limits: AgentResourceLimits,
    pub grant_token_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedAgentAccess {
    pub schema_version: u32,
    pub next_audit_sequence: u64,
    pub grants: Vec<AgentGrantRecord>,
}

impl Default for PersistedAgentAccess {
    fn default() -> Self {
        Self {
            schema_version: AGENT_ACCESS_SCHEMA_VERSION,
            next_audit_sequence: 0,
            grants: Vec::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentAuditEvent<'a> {
    schema_version: u32,
    sequence: u64,
    event_id: String,
    event_type: &'a str,
    grant_id: &'a str,
    at_unix_ms: u64,
    actor: &'static str,
}

pub struct AgentAccessStore {
    directory: PathBuf,
}

impl AgentAccessStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn read(&self) -> Result<PersistedAgentAccess, String> {
        ensure_directory(&self.directory, "Agent access configuration")?;
        let path = self.directory.join(CONFIG_FILENAME);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PersistedAgentAccess::default())
            }
            Err(error) => {
                return Err(format!(
                    "Agent access configuration cannot be inspected: {error}"
                ))
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("Agent access configuration is not a regular file".to_owned());
        }
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err("Agent access configuration exceeds 256 KiB".to_owned());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        File::open(&path)
            .and_then(|file| file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|error| format!("Agent access configuration cannot be read: {error}"))?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err("Agent access configuration exceeds 256 KiB".to_owned());
        }
        let state: PersistedAgentAccess = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Agent access configuration JSON is invalid: {error}"))?;
        validate_state(&state)?;
        Ok(state)
    }

    pub fn write_change(
        &self,
        state: &PersistedAgentAccess,
        event_type: &str,
        grant_id: &str,
        at_unix_ms: u64,
    ) -> Result<(), String> {
        validate_state(state)?;
        validate_safe_id("Agent audit event type", event_type)?;
        validate_safe_id("Agent grant id", grant_id)?;
        ensure_directory(&self.directory, "Agent access configuration")?;
        let audit_directory = self.directory.join(AUDIT_DIRECTORY);
        ensure_directory(&audit_directory, "Agent access audit")?;
        let event_id = Uuid::new_v4().simple().to_string();
        let event = AgentAuditEvent {
            schema_version: AGENT_ACCESS_SCHEMA_VERSION,
            sequence: state.next_audit_sequence,
            event_id: event_id.clone(),
            event_type,
            grant_id,
            at_unix_ms,
            actor: "desktop-user",
        };
        write_new_json(
            &audit_directory.join(format!("{:020}-{event_id}.json", state.next_audit_sequence)),
            &event,
            "Agent access audit",
        )?;
        self.write_state(state)
    }

    fn write_state(&self, state: &PersistedAgentAccess) -> Result<(), String> {
        let destination = self.directory.join(CONFIG_FILENAME);
        validate_file_destination(&destination, "Agent access configuration")?;
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| format!("Agent access configuration cannot be serialized: {error}"))?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err("Agent access configuration exceeds 256 KiB".to_owned());
        }
        let temporary = self
            .directory
            .join(format!(".agent-access-v1-{}.tmp", Uuid::new_v4().simple()));
        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| {
                    format!("Temporary Agent access configuration cannot be created: {error}")
                })?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| {
                    format!("Temporary Agent access configuration cannot be synced: {error}")
                })?;
            validate_file_destination(&destination, "Agent access configuration")?;
            fs::rename(&temporary, &destination).map_err(|error| {
                format!("Agent access configuration cannot be replaced: {error}")
            })?;
            File::open(&self.directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| {
                    format!("Agent access configuration directory cannot be synced: {error}")
                })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn validate_state(state: &PersistedAgentAccess) -> Result<(), String> {
    if state.schema_version != AGENT_ACCESS_SCHEMA_VERSION {
        return Err("Agent access configuration schema is unsupported".to_owned());
    }
    if state.grants.len() > MAX_GRANTS {
        return Err("At most 32 Agent grants are allowed".to_owned());
    }
    let catalog = tool_catalog()
        .iter()
        .map(|tool| tool.tool_id)
        .collect::<HashSet<_>>();
    let mut grant_ids = HashSet::with_capacity(state.grants.len());
    for grant in &state.grants {
        validate_safe_id("Agent grant id", &grant.grant_id)?;
        validate_safe_id("Agent id", &grant.agent_id)?;
        if grant.label.is_empty()
            || grant.label.chars().count() > 256
            || grant.label.chars().any(char::is_control)
        {
            return Err("Persisted Agent label is invalid".to_owned());
        }
        if !grant_ids.insert(grant.grant_id.as_str()) {
            return Err("Agent grant IDs must be unique".to_owned());
        }
        if grant.tool_ids.is_empty() || grant.tool_ids.len() > catalog.len() {
            return Err("Persisted Agent tool set is invalid".to_owned());
        }
        let mut tools = HashSet::with_capacity(grant.tool_ids.len());
        if grant
            .tool_ids
            .iter()
            .any(|tool| !catalog.contains(tool.as_str()) || !tools.insert(tool.as_str()))
        {
            return Err("Persisted Agent tool set is invalid".to_owned());
        }
        if grant.scopes.is_empty() || grant.scopes.len() > MAX_SCOPES {
            return Err("Persisted Agent scope set is invalid".to_owned());
        }
        let mut scopes = HashSet::with_capacity(grant.scopes.len());
        for scope in &grant.scopes {
            validate_safe_id("Agent scope id", &scope.scope_id)?;
            if !scopes.insert(scope.scope_id.as_str())
                || scope.display_path.is_empty()
                || scope.display_path.chars().any(char::is_control)
            {
                return Err("Persisted Agent scope is invalid".to_owned());
            }
        }
        if grant.expires_at_unix_ms <= grant.created_at_unix_ms {
            return Err("Persisted Agent grant expiry is invalid".to_owned());
        }
        let ttl_ms = grant.expires_at_unix_ms - grant.created_at_unix_ms;
        if !(MIN_GRANT_TTL_SECONDS * 1000..=MAX_GRANT_TTL_SECONDS * 1000).contains(&ttl_ms) {
            return Err("Persisted Agent grant TTL is invalid".to_owned());
        }
        if grant
            .revoked_at_unix_ms
            .is_some_and(|revoked| revoked < grant.created_at_unix_ms)
        {
            return Err("Persisted Agent revocation time is invalid".to_owned());
        }
        grant.limits.validate()?;
        if grant.grant_token_sha256.len() != 64
            || !grant
                .grant_token_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("Persisted Agent token digest is invalid".to_owned());
        }
    }
    Ok(())
}

fn ensure_directory(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(format!("{label} directory is not a trusted directory"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(path)
            .map_err(|error| format!("{label} directory cannot be created: {error}")),
        Err(error) => Err(format!("{label} directory cannot be inspected: {error}")),
    }
}

fn validate_file_destination(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(format!("{label} is not a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{label} cannot be inspected: {error}")),
    }
}

fn write_new_json(path: &Path, value: &impl Serialize, label: &str) -> Result<(), String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| format!("{label} cannot be serialized: {error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{label} cannot be created: {error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("{label} cannot be synchronized: {error}"))
}

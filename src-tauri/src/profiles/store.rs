use super::schema::{
    parse_candidate_profile, ClassificationRule, DeclarativeProfile, ProfileOwnership,
    ProfileProvenance, ProfileStatus, MAX_PROFILE_BYTES,
};
use crate::identity::ContentIdentity;
use crate::vault::records::{read_bytes_bounded, read_json, write_new_bytes, write_new_json};
use crate::vault::VaultLease;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PROFILE_RECORDS: usize = 1_000;
const PROFILE_RECORD_SCHEMA_VERSION: u32 = 1;
const NINEBOT_PROFILE_ID: &str = "ninebot-electronic-archive";
const NINEBOT_DRAFT_VERSION: &str = "0.1.0-draft";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileDecision {
    Approve,
    Reject,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSourceKind {
    LocalFile,
    RemoteUrl,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateStatus {
    Unapproved,
    Approved,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileVersionRef {
    pub profile_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileSummary {
    pub profile_id: String,
    pub version: String,
    pub title: String,
    pub status: ProfileStatus,
    pub rule_count: usize,
    pub provenance_title: String,
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDiff {
    pub added_rule_ids: Vec<String>,
    pub removed_rule_ids: Vec<String>,
    pub changed_rule_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileDecisionSummary {
    pub actor: String,
    pub decided_at_unix_ms: u64,
    pub decision: ProfileDecision,
    pub reviewed_digest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileCandidateRecord {
    pub schema_version: u32,
    pub candidate_id: String,
    pub imported_at_unix_ms: u64,
    pub source_kind: ProfileSourceKind,
    pub source_basename: String,
    #[serde(default)]
    pub source_byte_size: u64,
    pub locator_identity: ContentIdentity,
    pub source_identity: ContentIdentity,
    pub profile_id: String,
    pub profile_version: String,
    pub status: CandidateStatus,
    pub base: Option<ProfileVersionRef>,
    pub diff: ProfileDiff,
    pub approval: Option<ProfileDecisionSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStateSummary {
    pub installed: Vec<ProfileSummary>,
    pub active: Option<ProfileVersionRef>,
    pub candidates: Vec<ProfileCandidateRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileDecisionRecord {
    schema_version: u32,
    candidate_id: String,
    profile_id: String,
    profile_version: String,
    actor: String,
    decided_at_unix_ms: u64,
    decision: ProfileDecision,
    reviewed_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileActivationRecord {
    schema_version: u32,
    activation_id: String,
    candidate_id: String,
    profile_id: String,
    profile_version: String,
    actor: String,
    activated_at_unix_ms: u64,
    reviewed_digest: String,
}

#[derive(Clone, Default)]
pub struct ProfileAuthority {
    operation: Arc<Mutex<()>>,
}

impl ProfileAuthority {
    pub(crate) fn inspect(&self, vault: &VaultLease) -> Result<ProfileStateSummary, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Profile authority is unavailable".to_owned())?;
        ensure_bundled_ninebot_draft(vault)?;
        reconcile_approved_decisions(vault)?;
        load_state(vault)
    }

    pub(crate) fn import_local_bytes(
        &self,
        vault: &VaultLease,
        source_basename: &str,
        source_locator: &str,
        bytes: &[u8],
        imported_at: SystemTime,
    ) -> Result<ProfileCandidateRecord, String> {
        self.import_bytes(
            vault,
            ProfileSourceKind::LocalFile,
            source_basename,
            source_locator,
            bytes,
            imported_at,
        )
    }

    pub(crate) fn import_remote_bytes(
        &self,
        vault: &VaultLease,
        source_basename: &str,
        source_locator: &str,
        bytes: &[u8],
        imported_at: SystemTime,
    ) -> Result<ProfileCandidateRecord, String> {
        self.import_bytes(
            vault,
            ProfileSourceKind::RemoteUrl,
            source_basename,
            source_locator,
            bytes,
            imported_at,
        )
    }

    fn import_bytes(
        &self,
        vault: &VaultLease,
        source_kind: ProfileSourceKind,
        source_basename: &str,
        source_locator: &str,
        bytes: &[u8],
        imported_at: SystemTime,
    ) -> Result<ProfileCandidateRecord, String> {
        let profile = parse_candidate_profile(bytes)?;
        validate_basename(source_basename)?;
        if source_locator.is_empty() || source_locator.len() > 32 * 1024 {
            return Err("Profile source locator is invalid".to_owned());
        }
        let source_identity = ContentIdentity::from_reader(Cursor::new(bytes))
            .map_err(|error| format!("Profile source cannot be hashed: {error}"))?;
        let locator_identity = ContentIdentity::from_reader(Cursor::new(source_locator.as_bytes()))
            .map_err(|error| format!("Profile locator cannot be hashed: {error}"))?;
        let imported_at_unix_ms = system_time_ms(imported_at)?;

        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Profile authority is unavailable".to_owned())?;
        ensure_bundled_ninebot_draft(vault)?;

        let base_profile =
            load_active_profile(vault)?.filter(|active| active.profile_id == profile.profile_id);
        let base = base_profile.as_ref().map(|active| ProfileVersionRef {
            profile_id: active.profile_id.clone(),
            version: active.version.clone(),
        });
        let diff = profile_diff(base_profile.as_ref(), &profile);
        let candidate_binding = match source_kind {
            ProfileSourceKind::LocalFile => format!(
                "{}:{}:{}",
                source_identity.digest, profile.profile_id, profile.version
            ),
            ProfileSourceKind::RemoteUrl => format!(
                "remoteUrl:{}:{}:{}:{}",
                source_identity.digest,
                profile.profile_id,
                profile.version,
                locator_identity.digest
            ),
        };
        let candidate_id = ContentIdentity::from_reader(Cursor::new(candidate_binding.as_bytes()))
            .map_err(|error| format!("Profile candidate id cannot be generated: {error}"))?
            .digest;
        let candidate = ProfileCandidateRecord {
            schema_version: PROFILE_RECORD_SCHEMA_VERSION,
            candidate_id: candidate_id.clone(),
            imported_at_unix_ms,
            source_kind,
            source_basename: source_basename.to_owned(),
            source_byte_size: bytes.len() as u64,
            locator_identity,
            source_identity: source_identity.clone(),
            profile_id: profile.profile_id,
            profile_version: profile.version,
            status: CandidateStatus::Unapproved,
            base,
            diff,
            approval: None,
        };

        let candidate_path = candidate_record_path(&candidate_id);
        match vault.directory.symlink_metadata(&candidate_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("Profile candidate record is not a regular file".to_owned())
            }
            Ok(_) => {
                let existing: ProfileCandidateRecord =
                    read_json(&vault.directory, &candidate_path)?;
                if !same_candidate_import(&existing, &candidate) {
                    return Err("Profile candidate id conflicts with existing state".to_owned());
                }
                return Ok(existing);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Profile candidate record cannot be inspected: {error}"
                ))
            }
        }

        let source_path = source_record_path(&source_identity.digest);
        write_or_verify_source(vault, &source_path, bytes)?;
        write_new_json(&vault.directory, &candidate_path, &candidate)?;
        Ok(candidate)
    }

    pub(crate) fn decide(
        &self,
        vault: &VaultLease,
        candidate_id: &str,
        reviewed_digest: &str,
        decision: ProfileDecision,
        decided_at: SystemTime,
    ) -> Result<ProfileStateSummary, String> {
        validate_hex_identity(candidate_id, "candidate")?;
        validate_hex_identity(reviewed_digest, "reviewed profile")?;
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Profile authority is unavailable".to_owned())?;
        ensure_bundled_ninebot_draft(vault)?;

        let candidate: ProfileCandidateRecord =
            read_json(&vault.directory, &candidate_record_path(candidate_id))?;
        if candidate.schema_version != PROFILE_RECORD_SCHEMA_VERSION
            || candidate.candidate_id != candidate_id
            || candidate.source_identity.digest != reviewed_digest
            || candidate.source_identity.algorithm != "SHA-256"
        {
            return Err("Profile decision does not match the reviewed candidate".to_owned());
        }
        let decision_path = decision_record_path(candidate_id);
        if vault.directory.symlink_metadata(&decision_path).is_ok() {
            return Err("Profile candidate has already received a decision".to_owned());
        }
        let decided_at_unix_ms = system_time_ms(decided_at)?;
        let record = ProfileDecisionRecord {
            schema_version: PROFILE_RECORD_SCHEMA_VERSION,
            candidate_id: candidate_id.to_owned(),
            profile_id: candidate.profile_id.clone(),
            profile_version: candidate.profile_version.clone(),
            actor: "desktop-user".to_owned(),
            decided_at_unix_ms,
            decision,
            reviewed_digest: reviewed_digest.to_owned(),
        };

        if decision == ProfileDecision::Approve {
            let source = read_bytes_bounded(
                &vault.directory,
                &source_record_path(reviewed_digest),
                MAX_PROFILE_BYTES,
            )?;
            let mut approved = parse_candidate_profile(&source)?;
            if approved.profile_id != candidate.profile_id
                || approved.version != candidate.profile_version
            {
                return Err("Stored profile source does not match its candidate".to_owned());
            }
            approved.status = ProfileStatus::Approved;
            approved.validate()?;
            write_or_verify_profile(vault, &approved)?;
        }

        write_new_json(&vault.directory, &decision_path, &record)?;
        if decision == ProfileDecision::Approve {
            ensure_activation(vault, &record)?;
        }
        load_state(vault)
    }
}

fn same_candidate_import(
    existing: &ProfileCandidateRecord,
    requested: &ProfileCandidateRecord,
) -> bool {
    existing.schema_version == requested.schema_version
        && existing.candidate_id == requested.candidate_id
        && existing.source_kind == requested.source_kind
        && existing.source_basename == requested.source_basename
        && (existing.source_byte_size == 0
            || existing.source_byte_size == requested.source_byte_size)
        && existing.locator_identity == requested.locator_identity
        && existing.source_identity == requested.source_identity
        && existing.profile_id == requested.profile_id
        && existing.profile_version == requested.profile_version
        && existing.status == CandidateStatus::Unapproved
        && existing.base == requested.base
        && existing.diff == requested.diff
        && existing.approval.is_none()
}

fn bundled_ninebot_draft() -> DeclarativeProfile {
    DeclarativeProfile {
        schema_version: 1,
        profile_id: NINEBOT_PROFILE_ID.to_owned(),
        version: NINEBOT_DRAFT_VERSION.to_owned(),
        title: "Ninebot electronic archive".to_owned(),
        status: ProfileStatus::Draft,
        provenance: ProfileProvenance {
            source_title: "AI Knowledge Sort clean implementation handoff".to_owned(),
            ownership: ProfileOwnership::FirstPartyAuthorized,
            evidence: vec!["RULE-005".to_owned()],
        },
        rules: Vec::new(),
    }
}

fn ensure_bundled_ninebot_draft(vault: &VaultLease) -> Result<(), String> {
    let profile = bundled_ninebot_draft();
    profile.validate()?;
    write_or_verify_profile(vault, &profile)
}

fn write_or_verify_profile(vault: &VaultLease, profile: &DeclarativeProfile) -> Result<(), String> {
    let path = installed_profile_path(&profile.profile_id, &profile.version);
    match vault.directory.symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Installed profile is not a regular file".to_owned())
        }
        Ok(_) => {
            let existing: DeclarativeProfile = read_json(&vault.directory, &path)?;
            if existing == *profile {
                Ok(())
            } else {
                Err("Installed profile version is immutable".to_owned())
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            write_new_json(&vault.directory, &path, profile)
        }
        Err(error) => Err(format!("Installed profile cannot be inspected: {error}")),
    }
}

fn write_or_verify_source(vault: &VaultLease, path: &Path, bytes: &[u8]) -> Result<(), String> {
    match vault.directory.symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Stored profile source is not a regular file".to_owned())
        }
        Ok(_) => {
            let existing = read_bytes_bounded(&vault.directory, path, MAX_PROFILE_BYTES)?;
            if existing == bytes {
                Ok(())
            } else {
                Err("Stored profile source digest conflicts with existing bytes".to_owned())
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            write_new_bytes(&vault.directory, path, bytes)
        }
        Err(error) => Err(format!(
            "Stored profile source cannot be inspected: {error}"
        )),
    }
}

fn reconcile_approved_decisions(vault: &VaultLease) -> Result<(), String> {
    for path in list_json_records(vault, ".aiks/profiles/decisions")? {
        let decision: ProfileDecisionRecord = read_json(&vault.directory, &path)?;
        if decision.schema_version != PROFILE_RECORD_SCHEMA_VERSION {
            return Err("Profile decision schema is unsupported".to_owned());
        }
        if decision.decision == ProfileDecision::Approve {
            ensure_activation(vault, &decision)?;
        }
    }
    Ok(())
}

fn ensure_activation(vault: &VaultLease, decision: &ProfileDecisionRecord) -> Result<(), String> {
    let installed = installed_profile_path(&decision.profile_id, &decision.profile_version);
    let profile: DeclarativeProfile = read_json(&vault.directory, &installed)?;
    if profile.status != ProfileStatus::Approved {
        return Err("Approved profile installation is missing".to_owned());
    }
    let path = activation_record_path(&decision.candidate_id);
    let activation = ProfileActivationRecord {
        schema_version: PROFILE_RECORD_SCHEMA_VERSION,
        activation_id: decision.candidate_id.clone(),
        candidate_id: decision.candidate_id.clone(),
        profile_id: decision.profile_id.clone(),
        profile_version: decision.profile_version.clone(),
        actor: decision.actor.clone(),
        activated_at_unix_ms: decision.decided_at_unix_ms,
        reviewed_digest: decision.reviewed_digest.clone(),
    };
    match vault.directory.symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err("Profile activation is not a regular file".to_owned())
        }
        Ok(_) => {
            let existing: ProfileActivationRecord = read_json(&vault.directory, &path)?;
            if existing.activation_id == activation.activation_id
                && existing.candidate_id == activation.candidate_id
                && existing.profile_id == activation.profile_id
                && existing.profile_version == activation.profile_version
                && existing.reviewed_digest == activation.reviewed_digest
            {
                Ok(())
            } else {
                Err("Profile activation is immutable".to_owned())
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            write_new_json(&vault.directory, &path, &activation)
        }
        Err(error) => Err(format!("Profile activation cannot be inspected: {error}")),
    }
}

fn load_state(vault: &VaultLease) -> Result<ProfileStateSummary, String> {
    let decisions = load_decisions(vault)?;
    let mut installed = Vec::new();
    for path in list_json_records(vault, ".aiks/profiles/installed")? {
        let profile: DeclarativeProfile = read_json(&vault.directory, &path)?;
        profile.validate()?;
        installed.push(ProfileSummary {
            profile_id: profile.profile_id,
            version: profile.version,
            title: profile.title,
            status: profile.status,
            rule_count: profile.rules.len(),
            provenance_title: profile.provenance.source_title,
        });
    }
    installed.sort_by(|left, right| {
        left.profile_id
            .cmp(&right.profile_id)
            .then(left.version.cmp(&right.version))
    });

    let mut candidates = Vec::new();
    for path in list_json_records(vault, ".aiks/profiles/candidates")? {
        let mut candidate: ProfileCandidateRecord = read_json(&vault.directory, &path)?;
        candidate.source_identity.validate()?;
        candidate.locator_identity.validate()?;
        if let Some(decision) = decisions.get(&candidate.candidate_id) {
            candidate.status = match decision.decision {
                ProfileDecision::Approve => CandidateStatus::Approved,
                ProfileDecision::Reject => CandidateStatus::Rejected,
            };
            candidate.approval = Some(ProfileDecisionSummary {
                actor: decision.actor.clone(),
                decided_at_unix_ms: decision.decided_at_unix_ms,
                decision: decision.decision,
                reviewed_digest: decision.reviewed_digest.clone(),
            });
        }
        candidates.push(candidate);
    }
    candidates.sort_by(|left, right| {
        right
            .imported_at_unix_ms
            .cmp(&left.imported_at_unix_ms)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });

    Ok(ProfileStateSummary {
        installed,
        active: latest_activation(vault)?,
        candidates,
    })
}

fn load_decisions(vault: &VaultLease) -> Result<BTreeMap<String, ProfileDecisionRecord>, String> {
    let mut decisions = BTreeMap::new();
    for path in list_json_records(vault, ".aiks/profiles/decisions")? {
        let decision: ProfileDecisionRecord = read_json(&vault.directory, &path)?;
        if decisions
            .insert(decision.candidate_id.clone(), decision)
            .is_some()
        {
            return Err("Profile candidate has duplicate decisions".to_owned());
        }
    }
    Ok(decisions)
}

fn latest_activation(vault: &VaultLease) -> Result<Option<ProfileVersionRef>, String> {
    let mut latest: Option<ProfileActivationRecord> = None;
    for path in list_json_records(vault, ".aiks/profiles/activations")? {
        let activation: ProfileActivationRecord = read_json(&vault.directory, &path)?;
        if activation.schema_version != PROFILE_RECORD_SCHEMA_VERSION {
            return Err("Profile activation schema is unsupported".to_owned());
        }
        let replace = match latest.as_ref() {
            Some(current) => {
                (activation.activated_at_unix_ms, &activation.activation_id)
                    > (current.activated_at_unix_ms, &current.activation_id)
            }
            None => true,
        };
        if replace {
            latest = Some(activation);
        }
    }
    Ok(latest.map(|activation| ProfileVersionRef {
        profile_id: activation.profile_id,
        version: activation.profile_version,
    }))
}

fn load_active_profile(vault: &VaultLease) -> Result<Option<DeclarativeProfile>, String> {
    let Some(active) = latest_activation(vault)? else {
        return Ok(None);
    };
    read_json(
        &vault.directory,
        &installed_profile_path(&active.profile_id, &active.version),
    )
    .map(Some)
}

fn profile_diff(base: Option<&DeclarativeProfile>, candidate: &DeclarativeProfile) -> ProfileDiff {
    let base_rules = base.map(rule_map).unwrap_or_default();
    let candidate_rules = rule_map(candidate);
    let base_ids = base_rules.keys().cloned().collect::<HashSet<_>>();
    let candidate_ids = candidate_rules.keys().cloned().collect::<HashSet<_>>();
    let mut added_rule_ids = candidate_ids
        .difference(&base_ids)
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut removed_rule_ids = base_ids
        .difference(&candidate_ids)
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let mut changed_rule_ids = base_ids
        .intersection(&candidate_ids)
        .filter(|rule_id| base_rules.get(**rule_id) != candidate_rules.get(**rule_id))
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    added_rule_ids.sort();
    removed_rule_ids.sort();
    changed_rule_ids.sort();
    ProfileDiff {
        added_rule_ids,
        removed_rule_ids,
        changed_rule_ids,
    }
}

fn rule_map(profile: &DeclarativeProfile) -> BTreeMap<&str, &ClassificationRule> {
    profile
        .rules
        .iter()
        .map(|rule| (rule.rule_id.as_str(), rule))
        .collect()
}

fn list_json_records(vault: &VaultLease, directory: &str) -> Result<Vec<PathBuf>, String> {
    let mut records = Vec::new();
    let entries = vault
        .directory
        .read_dir(directory)
        .map_err(|error| format!("Profile record directory cannot be read: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Profile record cannot be listed: {error}"))?;
        if records.len() >= MAX_PROFILE_RECORDS {
            return Err("Profile record count exceeds the limit".to_owned());
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Profile record type cannot be read: {error}"))?;
        if file_type.is_symlink() {
            return Err("Profile record directory contains a link".to_owned());
        }
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        if Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
            != Some("json")
        {
            continue;
        }
        records.push(Path::new(directory).join(name));
    }
    records.sort();
    Ok(records)
}

fn validate_basename(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > 255
        || value.chars().any(char::is_control)
        || Path::new(value).file_name().and_then(|name| name.to_str()) != Some(value)
    {
        Err("Profile source basename is invalid".to_owned())
    } else {
        Ok(())
    }
}

fn validate_hex_identity(value: &str, label: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Err(format!("{label} digest is invalid"))
    } else {
        Ok(())
    }
}

fn system_time_ms(time: SystemTime) -> Result<u64, String> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|_| "Profile event time is before the Unix epoch".to_owned())?
        .as_millis()
        .try_into()
        .map_err(|_| "Profile event time is out of range".to_owned())
}

fn source_record_path(digest: &str) -> PathBuf {
    Path::new(".aiks/profiles/sources").join(format!("{digest}.json"))
}

fn candidate_record_path(candidate_id: &str) -> PathBuf {
    Path::new(".aiks/profiles/candidates").join(format!("{candidate_id}.json"))
}

fn decision_record_path(candidate_id: &str) -> PathBuf {
    Path::new(".aiks/profiles/decisions").join(format!("{candidate_id}.json"))
}

fn activation_record_path(candidate_id: &str) -> PathBuf {
    Path::new(".aiks/profiles/activations").join(format!("{candidate_id}.json"))
}

fn installed_profile_path(profile_id: &str, version: &str) -> PathBuf {
    Path::new(".aiks/profiles/installed").join(format!("{profile_id}--{version}.json"))
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateStatus, ProfileAuthority, ProfileCandidateRecord, ProfileDecision,
        ProfileSourceKind,
    };
    use crate::profiles::schema::ProfileStatus;
    use crate::vault::VaultAuthorityRegistry;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestVault {
        root: PathBuf,
        registry: VaultAuthorityRegistry,
    }

    impl TestVault {
        fn new() -> Self {
            let unique = format!(
                "aiknowledgesort-profiles-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after epoch")
                    .as_nanos(),
                NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed),
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir(&root).expect("create generated profile Vault");
            let root = root.canonicalize().expect("canonical profile Vault");
            let registry = VaultAuthorityRegistry::default();
            registry
                .authorize_path(&root)
                .expect("authorize profile Vault");
            Self { root, registry }
        }

        fn lease(&self) -> crate::vault::VaultLease {
            let summary = self.registry.current_summary().expect("current Vault");
            self.registry
                .lease(&summary.authority_id)
                .expect("lease profile Vault")
        }
    }

    impl Drop for TestVault {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove generated profile Vault");
        }
    }

    fn candidate_bytes(profile_id: &str, version: &str, rule_id: &str) -> Vec<u8> {
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "profileId": profile_id,
            "version": version,
            "title": "Imported fixture",
            "status": "candidate",
            "provenance": {
                "sourceTitle": "Owned fixture",
                "ownership": "owned",
                "evidence": ["authorization:test"]
            },
            "rules": [{
                "ruleId": rule_id,
                "destination": ["01-Research", "Reports"],
                "allOf": [{
                    "kind": "documentText",
                    "term": "quarterly report"
                }]
            }]
        }))
        .expect("serialize candidate profile")
    }

    fn directory_is_empty(path: &Path) -> bool {
        path.read_dir()
            .expect("read generated directory")
            .next()
            .is_none()
    }

    #[test]
    fn installs_a_non_active_manifest_only_ninebot_draft() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();

        let state = authority.inspect(&vault.lease()).expect("inspect profiles");

        let ninebot = state
            .installed
            .iter()
            .find(|profile| profile.profile_id == "ninebot-electronic-archive")
            .expect("Ninebot draft shell");
        assert_eq!(ninebot.status, ProfileStatus::Draft);
        assert_eq!(ninebot.rule_count, 0);
        assert!(state.active.is_none());
    }

    #[test]
    fn imports_exact_bytes_as_one_idempotent_unapproved_candidate() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        let bytes = candidate_bytes("fixture-profile", "1.0.0", "fixture.report");
        let locator = "/private/source/owned-profile.json";

        let first = authority
            .import_local_bytes(
                &vault.lease(),
                "owned-profile.json",
                locator,
                &bytes,
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            )
            .expect("import profile candidate");
        let repeated = authority
            .import_local_bytes(
                &vault.lease(),
                "owned-profile.json",
                locator,
                &bytes,
                SystemTime::UNIX_EPOCH,
            )
            .expect("repeat identical import");

        assert_eq!(first, repeated);
        assert_eq!(first.source_identity.algorithm, "SHA-256");
        assert_eq!(first.profile_id, "fixture-profile");
        assert_eq!(first.profile_version, "1.0.0");
        assert_eq!(first.diff.added_rule_ids, ["fixture.report"]);
        assert!(first.approval.is_none());
        assert_eq!(
            fs::read(
                vault
                    .root
                    .join(".aiks/profiles/sources")
                    .join(format!("{}.json", first.source_identity.digest))
            )
            .expect("read exact imported source"),
            bytes
        );
        let persisted = fs::read_to_string(
            vault
                .root
                .join(".aiks/profiles/candidates")
                .join(format!("{}.json", first.candidate_id)),
        )
        .expect("read candidate record");
        assert!(!persisted.contains(locator));
        assert!(persisted.contains("owned-profile.json"));
    }

    #[test]
    fn imports_remote_bytes_as_a_separate_unapproved_candidate_without_locator_secrets() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        let bytes = candidate_bytes("remote-profile", "1.0.0", "remote.rule");
        let remote = authority
            .import_remote_bytes(
                &vault.lease(),
                "remote-profile.json",
                "https://profiles.example.com/remote-profile.json?signature=synthetic-secret",
                &bytes,
                SystemTime::UNIX_EPOCH,
            )
            .expect("import remote profile candidate");
        let local = authority
            .import_local_bytes(
                &vault.lease(),
                "remote-profile.json",
                "/private/remote-profile.json",
                &bytes,
                SystemTime::UNIX_EPOCH,
            )
            .expect("import local profile candidate");

        assert_eq!(remote.source_kind, ProfileSourceKind::RemoteUrl);
        assert_eq!(remote.source_byte_size, bytes.len() as u64);
        assert_eq!(remote.status, CandidateStatus::Unapproved);
        assert_ne!(remote.candidate_id, local.candidate_id);
        assert!(authority.inspect(&vault.lease()).unwrap().active.is_none());

        let persisted = fs::read_to_string(
            vault
                .root
                .join(".aiks/profiles/candidates")
                .join(format!("{}.json", remote.candidate_id)),
        )
        .expect("read remote candidate record");
        assert!(!persisted.contains("profiles.example.com"));
        assert!(!persisted.contains("synthetic-secret"));
    }

    #[test]
    fn decodes_candidate_records_written_before_source_size_was_added() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        let bytes = candidate_bytes("legacy-profile", "1.0.0", "legacy.rule");
        let candidate = authority
            .import_local_bytes(
                &vault.lease(),
                "legacy-profile.json",
                "/private/legacy-profile.json",
                &bytes,
                SystemTime::UNIX_EPOCH,
            )
            .expect("import candidate fixture");
        let mut legacy = serde_json::to_value(candidate).expect("serialize candidate fixture");
        legacy
            .as_object_mut()
            .expect("candidate object")
            .remove("sourceByteSize");

        let decoded: ProfileCandidateRecord =
            serde_json::from_value(legacy).expect("decode legacy candidate record");

        assert_eq!(decoded.source_byte_size, 0);
    }

    #[test]
    fn rejects_invalid_or_executable_shapes_without_candidate_mutation() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        authority
            .inspect(&vault.lease())
            .expect("initialize profiles");
        let executable = br#"{
            "schemaVersion":1,
            "profileId":"bad",
            "version":"1",
            "title":"Bad",
            "status":"candidate",
            "command":"run",
            "provenance":{"sourceTitle":"Bad","ownership":"owned","evidence":["test"]},
            "rules":[]
        }"#;

        assert!(authority
            .import_local_bytes(
                &vault.lease(),
                "bad.json",
                "/private/bad.json",
                executable,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        assert!(directory_is_empty(
            &vault.root.join(".aiks/profiles/sources")
        ));
        assert!(directory_is_empty(
            &vault.root.join(".aiks/profiles/candidates")
        ));
        assert!(directory_is_empty(
            &vault.root.join(".aiks/profiles/decisions")
        ));
        assert!(directory_is_empty(
            &vault.root.join(".aiks/profiles/activations")
        ));
    }

    #[test]
    fn records_exact_non_replayable_approval_and_rejection_decisions() {
        let vault = TestVault::new();
        let authority = ProfileAuthority::default();
        let approved = authority
            .import_local_bytes(
                &vault.lease(),
                "approved.json",
                "/private/approved.json",
                &candidate_bytes("approved-profile", "1.0.0", "approved.rule"),
                SystemTime::UNIX_EPOCH,
            )
            .expect("import approval candidate");

        let state = authority
            .decide(
                &vault.lease(),
                &approved.candidate_id,
                &approved.source_identity.digest,
                ProfileDecision::Approve,
                SystemTime::UNIX_EPOCH,
            )
            .expect("approve candidate");

        assert_eq!(
            state.active.expect("active approved profile").profile_id,
            "approved-profile"
        );
        assert!(authority
            .decide(
                &vault.lease(),
                &approved.candidate_id,
                &approved.source_identity.digest,
                ProfileDecision::Approve,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());

        let rejected = authority
            .import_local_bytes(
                &vault.lease(),
                "rejected.json",
                "/private/rejected.json",
                &candidate_bytes("rejected-profile", "1.0.0", "rejected.rule"),
                SystemTime::UNIX_EPOCH,
            )
            .expect("import rejection candidate");
        assert!(authority
            .decide(
                &vault.lease(),
                &rejected.candidate_id,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                ProfileDecision::Reject,
                SystemTime::UNIX_EPOCH,
            )
            .is_err());
        let state = authority
            .decide(
                &vault.lease(),
                &rejected.candidate_id,
                &rejected.source_identity.digest,
                ProfileDecision::Reject,
                SystemTime::UNIX_EPOCH,
            )
            .expect("reject candidate");
        assert_eq!(
            state.active.expect("approval remains active").profile_id,
            "approved-profile"
        );
    }
}

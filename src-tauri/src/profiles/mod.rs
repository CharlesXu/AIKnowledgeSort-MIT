pub mod proposal;
mod remote;
pub mod schema;
mod store;

use crate::discovery::{open_trusted_drop_root, CapabilityRoot};
use crate::vault::VaultAuthorityRegistry;
use schema::MAX_PROFILE_BYTES;
use serde::Deserialize;
use std::io::Read;
use std::time::SystemTime;

pub use store::{
    CandidateStatus, ProfileAuthority, ProfileCandidateRecord, ProfileDecision,
    ProfileDecisionSummary, ProfileDiff, ProfileSourceKind, ProfileStateSummary, ProfileSummary,
    ProfileVersionRef,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecideProfileCandidateRequest {
    candidate_id: String,
    reviewed_digest: String,
    decision: ProfileDecision,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportUrlProfileCandidateRequest {
    url: String,
}

fn current_vault(registry: &VaultAuthorityRegistry) -> Result<crate::vault::VaultLease, String> {
    let summary = registry.current_summary()?;
    registry.lease(&summary.authority_id)
}

#[tauri::command]
pub fn inspect_profile_state(
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
) -> Result<ProfileStateSummary, String> {
    let vault = current_vault(vaults.inner())?;
    profiles.inspect(&vault)
}

#[tauri::command]
pub fn import_local_profile_candidate(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
) -> Result<Option<ProfileCandidateRecord>, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(selected) = app.dialog().file().blocking_pick_file() else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected profile path is unavailable: {error}"))?;
    let source_basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Selected profile filename is unavailable".to_owned())?
        .to_owned();
    let source_locator = path.to_string_lossy().into_owned();
    let file = match open_trusted_drop_root(path) {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Selected profile must be a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { message, .. } => return Err(message),
    };
    let byte_size = file
        .metadata()
        .map_err(|error| format!("Selected profile metadata is unavailable: {error}"))?
        .len();
    if byte_size == 0 || byte_size > MAX_PROFILE_BYTES as u64 {
        return Err("Profile input is empty or exceeds 1 MiB".to_owned());
    }
    let mut bytes = Vec::with_capacity(byte_size as usize);
    file.take(MAX_PROFILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Selected profile cannot be read: {error}"))?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err("Profile input exceeds 1 MiB".to_owned());
    }

    let vault = current_vault(vaults.inner())?;
    profiles
        .import_local_bytes(
            &vault,
            &source_basename,
            &source_locator,
            &bytes,
            SystemTime::now(),
        )
        .map(Some)
}

#[tauri::command]
pub async fn import_url_profile_candidate(
    request: ImportUrlProfileCandidateRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
) -> Result<ProfileCandidateRecord, String> {
    let fetched = remote::fetch_profile_url(&request.url).await?;
    let vault = current_vault(vaults.inner())?;
    profiles.import_remote_bytes(
        &vault,
        &fetched.source_basename,
        &fetched.minimized_locator,
        &fetched.bytes,
        SystemTime::now(),
    )
}

#[tauri::command]
pub fn decide_profile_candidate(
    request: DecideProfileCandidateRequest,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
) -> Result<ProfileStateSummary, String> {
    let vault = current_vault(vaults.inner())?;
    profiles.decide(
        &vault,
        &request.candidate_id,
        &request.reviewed_digest,
        request.decision,
        SystemTime::now(),
    )
}

mod classification;
mod compiler;
mod ninebot;
pub mod proposal;
mod remote;
pub mod schema;
mod store;

use crate::discovery::{open_trusted_drop_root, CapabilityRoot, ReviewedSourceRegistry};
use crate::vault::VaultAuthorityRegistry;
use schema::MAX_PROFILE_BYTES;
use serde::Deserialize;
use std::io::Read;
use std::time::SystemTime;

pub use classification::{
    ClassificationBatch, ClassificationBatchItem, ClassificationBatchRegistry,
    ClassificationItemInput,
};
pub use compiler::CompileProfileCandidateRequest;
pub use store::{
    CandidateStatus, ProfileAuthority, ProfileCandidateRecord, ProfileDecision,
    ProfileDecisionSummary, ProfileDiff, ProfileGenerationSummary, ProfileSourceKind,
    ProfileStateSummary, ProfileSummary, ProfileTaxonomyCounts, ProfileVersionRef,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateClassificationBatchRequest {
    proposal_id: String,
    items: Vec<ClassificationItemInput>,
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
pub(crate) fn create_classification_batch(
    request: CreateClassificationBatchRequest,
    reviewed_sources: tauri::State<'_, ReviewedSourceRegistry>,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
    batches: tauri::State<'_, ClassificationBatchRegistry>,
) -> Result<ClassificationBatch, String> {
    let now = std::time::Instant::now();
    let item_ids = request
        .items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<Vec<_>>();
    let sources = reviewed_sources.resolve_selection_at(&request.proposal_id, &item_ids, now)?;
    let vault = current_vault(vaults.inner())?;
    let profile = profiles.active_approved_profile(&vault)?;
    let semantic_count = request
        .items
        .iter()
        .filter(|item| item.semantic_comparison_id.is_some())
        .count();
    if semantic_count > 0 {
        if semantic_count != request.items.len()
            || request.items.iter().any(|item| !item.references.is_empty())
        {
            return Err(
                "A classification batch cannot mix semantic decisions with supplied evidence"
                    .to_owned(),
            );
        }
        let comparisons = request
            .items
            .iter()
            .map(|item| {
                crate::model_runtime::file_semantics::load_file_semantic_comparison(
                    &vault,
                    item.semantic_comparison_id
                        .as_deref()
                        .expect("semantic comparison exists"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return batches.create_semantic_at(
            &request.proposal_id,
            &profile,
            sources,
            comparisons,
            now,
            SystemTime::now(),
        );
    }
    batches.create_at(
        &request.proposal_id,
        &profile,
        sources,
        request.items,
        now,
        SystemTime::now(),
    )
}

#[tauri::command]
pub async fn import_local_profile_candidate(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
) -> Result<Option<ProfileCandidateRecord>, String> {
    let Some(selected) = crate::native_dialog::pick_file(&app).await? else {
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
pub async fn compile_local_profile_candidate(
    request: CompileProfileCandidateRequest,
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultAuthorityRegistry>,
    profiles: tauri::State<'_, ProfileAuthority>,
    models: tauri::State<'_, crate::model_runtime::ModelRuntimeAuthority>,
) -> Result<Option<ProfileCandidateRecord>, String> {
    let Some(selected) = crate::native_dialog::pick_file(&app).await? else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| format!("Selected compiler source is unavailable: {error}"))?;
    let source_basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Selected compiler source filename is unavailable".to_owned())?
        .to_owned();
    let file = match open_trusted_drop_root(path) {
        CapabilityRoot::File { file, .. } => file,
        CapabilityRoot::Directory { .. } => {
            return Err("Compiler source must be a regular file".to_owned())
        }
        CapabilityRoot::Diagnostic { message, .. } => return Err(message),
    };
    let byte_size = file
        .metadata()
        .map_err(|error| format!("Compiler source metadata is unavailable: {error}"))?
        .len();
    if byte_size == 0 || byte_size > compiler::MAX_COMPILER_SOURCE_BYTES as u64 {
        return Err("Compiler source is empty or exceeds 512 KiB".to_owned());
    }
    let mut source_bytes = Vec::with_capacity(byte_size as usize);
    file.take(compiler::MAX_COMPILER_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut source_bytes)
        .map_err(|error| format!("Compiler source cannot be read: {error}"))?;
    if source_bytes.len() > compiler::MAX_COMPILER_SOURCE_BYTES {
        return Err("Compiler source exceeds 512 KiB".to_owned());
    }

    let vault = current_vault(vaults.inner())?;
    let base = profiles.profile_by_version_read_only(
        &vault,
        &request.base_profile_id,
        &request.base_profile_version,
    )?;
    let config = models.load_config(
        crate::model_runtime::app_config_directory(&app)?,
        &request.config_id,
    )?;
    let profiles = profiles.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let compiled = compiler::compile_candidate(
            &request,
            &source_basename,
            &source_bytes,
            &base,
            &config,
            &compiler::OpenAiProfileCompiler,
        )?;
        let generated_basename =
            format!("{}--{}.profile.json", request.profile_id, request.version);
        let generation = ProfileGenerationSummary {
            original_source_basename: source_basename,
            original_source_byte_size: source_bytes.len() as u64,
            original_source_identity: compiled.source_identity,
            model_config_id: config.config_id,
            model: config.model,
            base: ProfileVersionRef {
                profile_id: base.profile_id,
                version: base.version,
            },
        };
        profiles.import_model_bytes(
            &vault,
            &generated_basename,
            &compiled.bytes,
            &source_bytes,
            generation,
            SystemTime::now(),
        )
    })
    .await
    .map_err(|error| format!("Profile compiler worker failed: {error}"))?
    .map(Some)
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

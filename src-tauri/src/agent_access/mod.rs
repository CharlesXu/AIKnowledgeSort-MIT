pub(crate) mod authority;
pub(crate) mod schema;
pub(crate) mod store;

use authority::AgentAccessAuthority;
use schema::{
    AgentAccessState, CreateAgentGrantRequest, IssuedAgentGrant, NativeScopeSelection,
    RevokeAgentGrantRequest,
};
use std::path::PathBuf;
use std::time::SystemTime;
use tauri::Manager;

fn app_config_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Agent access configuration directory is unavailable: {error}"))
}

#[tauri::command]
pub async fn select_agent_grant_directories(
    app: tauri::AppHandle,
    authority: tauri::State<'_, AgentAccessAuthority>,
) -> Result<Option<NativeScopeSelection>, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(selected) = app.dialog().file().blocking_pick_folders() else {
        return Ok(None);
    };
    let paths = selected
        .into_iter()
        .map(|path| {
            path.into_path()
                .map_err(|error| format!("Selected Agent scope path is unavailable: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    authority.select_paths(paths, SystemTime::now()).map(Some)
}

#[tauri::command]
pub fn inspect_agent_access(
    app: tauri::AppHandle,
    authority: tauri::State<'_, AgentAccessAuthority>,
) -> Result<AgentAccessState, String> {
    authority.inspect(&app_config_directory(&app)?, SystemTime::now())
}

#[tauri::command]
pub fn create_agent_grant(
    app: tauri::AppHandle,
    authority: tauri::State<'_, AgentAccessAuthority>,
    request: CreateAgentGrantRequest,
) -> Result<IssuedAgentGrant, String> {
    authority.create_grant(&app_config_directory(&app)?, request, SystemTime::now())
}

#[tauri::command]
pub fn revoke_agent_grant(
    app: tauri::AppHandle,
    authority: tauri::State<'_, AgentAccessAuthority>,
    request: RevokeAgentGrantRequest,
) -> Result<AgentAccessState, String> {
    authority.revoke(
        &app_config_directory(&app)?,
        &request.grant_id,
        SystemTime::now(),
    )
}

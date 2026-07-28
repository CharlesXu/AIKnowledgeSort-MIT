mod auth;
mod http;
mod service;
mod tools;

pub use http::{McpTransportAuthority, McpTransportState, StartMcpTransportRequest};
pub use service::GovernedMcpService;

use crate::agent_access::authority::AgentAccessAuthority;
use std::path::PathBuf;
use tauri::Manager;

fn app_config_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("MCP configuration directory is unavailable: {error}"))
}

#[tauri::command]
pub async fn inspect_mcp_transport(
    transport: tauri::State<'_, McpTransportAuthority>,
) -> Result<McpTransportState, String> {
    transport.inspect().await
}

#[tauri::command]
pub async fn start_mcp_transport(
    app: tauri::AppHandle,
    agent_access: tauri::State<'_, AgentAccessAuthority>,
    transport: tauri::State<'_, McpTransportAuthority>,
    request: StartMcpTransportRequest,
) -> Result<McpTransportState, String> {
    transport
        .start(
            agent_access.inner().clone(),
            app_config_directory(&app)?,
            request.port,
        )
        .await
}

#[tauri::command]
pub async fn stop_mcp_transport(
    transport: tauri::State<'_, McpTransportAuthority>,
) -> Result<McpTransportState, String> {
    transport.stop().await
}

mod config;

use config::{ModelConfigInput, ModelConfigStore, ModelRuntimeState};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Clone, Default)]
pub struct ModelRuntimeAuthority {
    operation: Arc<Mutex<()>>,
}

impl ModelRuntimeAuthority {
    fn inspect(&self, directory: PathBuf) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).inspect()
    }

    fn upsert(
        &self,
        directory: PathBuf,
        request: ModelConfigInput,
    ) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).upsert(request)
    }

    fn remove(&self, directory: PathBuf, config_id: &str) -> Result<ModelRuntimeState, String> {
        let _operation = self
            .operation
            .lock()
            .map_err(|_| "Model runtime authority is unavailable".to_owned())?;
        ModelConfigStore::new(directory).remove(config_id)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveModelConfigRequest {
    config_id: String,
}

fn app_config_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Application configuration directory is unavailable: {error}"))
}

#[tauri::command]
pub fn inspect_model_runtime(
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.inspect(app_config_directory(&app)?)
}

#[tauri::command]
pub fn upsert_model_config(
    request: ModelConfigInput,
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.upsert(app_config_directory(&app)?, request)
}

#[tauri::command]
pub fn remove_model_config(
    request: RemoveModelConfigRequest,
    app: tauri::AppHandle,
    authority: tauri::State<'_, ModelRuntimeAuthority>,
) -> Result<ModelRuntimeState, String> {
    authority.remove(app_config_directory(&app)?, &request.config_id)
}

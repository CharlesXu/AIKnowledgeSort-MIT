use tauri_plugin_dialog::{DialogExt, FilePath};

pub async fn pick_file(app: &tauri::AppHandle) -> Result<Option<FilePath>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_file(move |selection| {
        let _ = sender.send(selection);
    });
    receiver
        .await
        .map_err(|_| "Native file selection did not complete".to_owned())
}

pub async fn pick_folder(app: &tauri::AppHandle) -> Result<Option<FilePath>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |selection| {
        let _ = sender.send(selection);
    });
    receiver
        .await
        .map_err(|_| "Native folder selection did not complete".to_owned())
}

pub async fn pick_folders(app: &tauri::AppHandle) -> Result<Option<Vec<FilePath>>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folders(move |selection| {
        let _ = sender.send(selection);
    });
    receiver
        .await
        .map_err(|_| "Native folder selection did not complete".to_owned())
}

fn application_name() -> &'static str {
    "AI Knowledge Sort"
}

#[path = "discovery/mod.rs"]
mod discovery;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(discovery::DropGrantRegistry::default())
        .on_window_event(|window, event| {
            use tauri::{Emitter, Manager};

            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let registry = window.state::<discovery::DropGrantRegistry>();
                match discovery::issue_drop_grant(&registry, paths.clone()) {
                    Ok(issued) => {
                        let _ = window.emit(discovery::DROP_GRANT_EVENT, issued);
                    }
                    Err(error) => {
                        let _ = window.emit(discovery::DROP_GRANT_ERROR_EVENT, error);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![discovery::propose_local_drop])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while running {}: {error}", application_name()));
}

#[cfg(test)]
mod tests {
    #[test]
    fn identifies_the_source_workbench() {
        assert_eq!(super::application_name(), "AI Knowledge Sort");
    }
}

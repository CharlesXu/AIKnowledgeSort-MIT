fn application_name() -> &'static str {
    "AI Knowledge Sort"
}

#[path = "discovery/mod.rs"]
mod discovery;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(discovery::DropGrantRegistry::default())
        .manage(discovery::DropWorkLimiter::default())
        .on_window_event(|window, event| {
            use tauri::{Emitter, Manager};

            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let limiter = window.state::<discovery::DropWorkLimiter>().inner().clone();
                let permit = match limiter.try_acquire() {
                    Ok(permit) => permit,
                    Err(error) => {
                        let _ = window.emit(discovery::DROP_GRANT_ERROR_EVENT, error);
                        return;
                    }
                };
                let registry = window
                    .state::<discovery::DropGrantRegistry>()
                    .inner()
                    .clone();
                let dropped_paths = paths.clone();
                let target = window.clone();

                tauri::async_runtime::spawn(async move {
                    let task = tauri::async_runtime::spawn_blocking(move || {
                        let _permit = permit;
                        discovery::issue_drop_grant(&registry, dropped_paths)
                    });
                    match tokio::time::timeout(discovery::DROP_WORK_TIMEOUT, task).await {
                        Ok(Ok(Ok(issued))) => {
                            let _ = target.emit(discovery::DROP_GRANT_EVENT, issued);
                        }
                        Ok(Ok(Err(error))) => {
                            let _ = target.emit(
                                discovery::DROP_GRANT_ERROR_EVENT,
                                discovery::bounded_error(error),
                            );
                        }
                        Ok(Err(error)) => {
                            let _ = target.emit(
                                discovery::DROP_GRANT_ERROR_EVENT,
                                discovery::bounded_error(format!(
                                    "Drop grant worker failed: {error}"
                                )),
                            );
                        }
                        Err(_) => {
                            let _ = target.emit(
                                discovery::DROP_GRANT_ERROR_EVENT,
                                "Drop grant processing deadline exceeded",
                            );
                        }
                    }
                });
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

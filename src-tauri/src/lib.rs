use std::ffi::{OsStr, OsString};

fn application_name() -> &'static str {
    "AI Knowledge Sort"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessMode {
    Desktop,
    DesktopSmoke,
    McpStdioRelay,
}

pub fn process_mode<I, S>(args: I) -> Result<ProcessMode, String>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let arguments = args.into_iter().skip(1).map(Into::into).collect::<Vec<_>>();
    let desktop_smoke = OsStr::new("--desktop-smoke");
    let mcp_relay = OsStr::new("--mcp-stdio-relay");
    let broker_url = OsStr::new("--broker-url");

    match arguments.as_slice() {
        [] => Ok(ProcessMode::Desktop),
        [switch] if switch == desktop_smoke => Ok(ProcessMode::DesktopSmoke),
        [relay, broker, _url] if relay == mcp_relay && broker == broker_url => {
            Ok(ProcessMode::McpStdioRelay)
        }
        _ if arguments
            .iter()
            .any(|argument| argument == desktop_smoke || argument == mcp_relay) =>
        {
            Err("Reserved process mode arguments are malformed or conflicting".to_owned())
        }
        _ => Ok(ProcessMode::Desktop),
    }
}

pub mod agent_access;
mod archive;
mod cleanup;
#[path = "discovery/mod.rs"]
mod discovery;
mod evidence_extraction;
mod graph;
pub mod identity;
mod knowledge;
pub mod mcp_transport;
mod model_runtime;
mod naming;
mod native_dialog;
pub mod profiles;
mod vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    run_application(false);
}

pub fn run_desktop_smoke() {
    run_application(true);
}

fn run_application(exit_when_ready: bool) {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(agent_access::authority::AgentAccessAuthority::default())
        .manage(mcp_transport::McpTransportAuthority::default())
        .manage(discovery::DropGrantRegistry::default())
        .manage(discovery::ReviewedSourceRegistry::default())
        .manage(discovery::DropWorkLimiter::default())
        .manage(vault::VaultAuthorityRegistry::default())
        .manage(archive::ArchivePlanRegistry::default())
        .manage(archive::ArchiveUndoPlanRegistry::default())
        .manage(cleanup::CleanupPlanRegistry::default())
        .manage(naming::NamingBatchRegistry::default())
        .manage(profiles::ClassificationBatchRegistry::default())
        .manage(knowledge::KnowledgeWriteRegistry::default())
        .manage(graph::GraphWriteRegistry::default())
        .manage(model_runtime::ModelRuntimeAuthority::default())
        .manage(profiles::ProfileAuthority::default())
        .on_window_event(|window, event| {
            use tauri::{Emitter, Manager};

            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let limiter = window.state::<discovery::DropWorkLimiter>().inner().clone();
                let registry = window
                    .state::<discovery::DropGrantRegistry>()
                    .inner()
                    .clone();
                let dropped_paths = paths.clone();
                let target = window.clone();

                tauri::async_runtime::spawn(async move {
                    match discovery::issue_local_source_grant(dropped_paths, registry, limiter)
                        .await
                    {
                        Ok(issued) => {
                            let _ = target.emit(discovery::DROP_GRANT_EVENT, issued);
                        }
                        Err(error) => {
                            let _ = target.emit(
                                discovery::DROP_GRANT_ERROR_EVENT,
                                discovery::bounded_error(error),
                            );
                        }
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            agent_access::select_agent_grant_directories,
            agent_access::inspect_agent_access,
            agent_access::create_agent_grant,
            agent_access::revoke_agent_grant,
            mcp_transport::inspect_mcp_transport,
            mcp_transport::start_mcp_transport,
            mcp_transport::stop_mcp_transport,
            discovery::choose_local_files,
            discovery::choose_local_folders,
            discovery::propose_local_drop,
            vault::choose_authoritative_vault,
            archive::create_archive_plan,
            archive::confirm_archive_plan,
            archive::undo::create_archive_undo_plan,
            archive::undo::confirm_archive_undo_plan,
            cleanup::create_cleanup_plan,
            cleanup::authorize_permanent_cleanup,
            cleanup::confirm_cleanup_plan,
            naming::create_naming_batch,
            knowledge::open_knowledge_document,
            knowledge::list_knowledge_targets,
            knowledge::save_knowledge_document,
            graph::inspect_knowledge_graph,
            graph::propose_graph_relation,
            graph::decide_graph_relation,
            model_runtime::inspect_model_runtime,
            model_runtime::discover_models,
            model_runtime::upsert_model_config,
            model_runtime::remove_model_config,
            model_runtime::run_model_comparison,
            model_runtime::run_file_semantic_comparison,
            profiles::inspect_profile_state,
            profiles::create_classification_batch,
            profiles::import_local_profile_candidate,
            profiles::import_url_profile_candidate,
            profiles::compile_local_profile_candidate,
            profiles::decide_profile_candidate
        ])
        .build(tauri::generate_context!())
        .unwrap_or_else(|error| panic!("error while building {}: {error}", application_name()));
    application.run(move |application, event| {
        if exit_when_ready && matches!(event, tauri::RunEvent::Ready) {
            application.exit(0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{process_mode, ProcessMode};

    #[test]
    fn identifies_the_source_workbench() {
        assert_eq!(super::application_name(), "AI Knowledge Sort");
    }

    #[test]
    fn process_mode_selects_each_reserved_entrypoint_exactly() {
        assert_eq!(
            process_mode(["app"].into_iter()).expect("normal desktop mode"),
            ProcessMode::Desktop
        );
        assert_eq!(
            process_mode(["app", "--desktop-smoke"].into_iter()).expect("desktop smoke mode"),
            ProcessMode::DesktopSmoke
        );
        assert_eq!(
            process_mode(
                [
                    "app",
                    "--mcp-stdio-relay",
                    "--broker-url",
                    "http://127.0.0.1:3000/mcp",
                ]
                .into_iter()
            )
            .expect("MCP stdio relay mode"),
            ProcessMode::McpStdioRelay
        );
    }

    #[test]
    fn process_mode_rejects_malformed_or_conflicting_reserved_switches() {
        assert!(process_mode(["app", "--desktop-smoke", "extra"].into_iter()).is_err());
        assert!(process_mode(["app", "--desktop-smoke", "--mcp-stdio-relay"].into_iter()).is_err());
        assert!(process_mode(
            [
                "app",
                "--mcp-stdio-relay",
                "--broker-url",
                "http://127.0.0.1:3000/mcp",
                "extra",
            ]
            .into_iter()
        )
        .is_err());
    }
}

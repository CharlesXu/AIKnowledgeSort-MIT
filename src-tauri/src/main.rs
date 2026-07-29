#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mode = match ai_knowledge_sort_lib::process_mode(std::env::args_os()) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    match mode {
        ai_knowledge_sort_lib::ProcessMode::Desktop => ai_knowledge_sort_lib::run(),
        ai_knowledge_sort_lib::ProcessMode::DesktopSmoke => {
            ai_knowledge_sort_lib::run_desktop_smoke()
        }
        ai_knowledge_sort_lib::ProcessMode::McpStdioRelay => {
            match ai_knowledge_sort_lib::mcp_transport::stdio_relay::maybe_run_from_process_args() {
                Ok(true) => {}
                Ok(false) => {
                    eprintln!("MCP stdio relay process mode was not recognized");
                    std::process::exit(2);
                }
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
        }
    }
}

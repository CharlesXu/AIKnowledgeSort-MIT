#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    match ai_knowledge_sort_lib::mcp_transport::stdio_relay::maybe_run_from_process_args() {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
    ai_knowledge_sort_lib::run();
}

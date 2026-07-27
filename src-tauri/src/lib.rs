fn application_name() -> &'static str {
    "AI Knowledge Sort"
}

mod discovery;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![discovery::propose_local_drop])
        .run(tauri::generate_context!())
        .unwrap_or_else(|error| {
            panic!("error while running {}: {error}", application_name())
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn identifies_the_source_workbench() {
        assert_eq!(super::application_name(), "AI Knowledge Sort");
    }
}

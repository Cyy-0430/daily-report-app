mod config;
mod export;
mod llm;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            llm::test_connection,
            llm::generate_report,
            export::export_report,
            export::write_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

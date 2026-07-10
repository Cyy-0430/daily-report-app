mod collector;
mod config;
mod db;
mod export;
mod llm;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // 数据目录 + 建库
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("daily_report.db");
            let conn = Connection::open(db_path)?;
            db::init_db(&conn).map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            // 一次性迁移旧 data.json(meta 未标记时才读 store、执行迁移)
            let need_migrate = db::get_meta(&conn, "migrated_from_store")
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?
                .is_none();
            if need_migrate {
                let legacy = db::read_legacy_from_store(app.handle())
                    .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
                db::migrate_from_store(&conn, legacy)
                    .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
            }

            app.manage(db::DbState(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            db::list_history,
            db::add_history,
            db::remove_history,
            llm::test_connection,
            llm::generate_report,
            collector::collect_conversations,
            export::export_report,
            export::write_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

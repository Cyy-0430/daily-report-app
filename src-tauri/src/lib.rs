mod collector;
mod config;
mod db;
mod export;
mod llm;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::Connection;
use tauri::Manager;

/// SQLite 数据库文件名(位于 app_data_dir)。
const DB_FILE_NAME: &str = "daily_report.db";

/// 主窗口是否已显示。窗口以 `visible: false` 创建,主页面首次加载完成才显示,
/// 把 WebView2 冷启动的空白期完全藏在幕后(窗口出现即就绪)。
/// 页面加载事件与超时兜底先到先得;AtomicBool 保证只显示一次
/// (dev 模式下 Vite 热更新整页 reload 会重复触发页面加载事件)。
static MAIN_WINDOW_SHOWN: AtomicBool = AtomicBool::new(false);

/// 显示主窗口,仅首次调用生效。
fn show_main_window_once(app: &tauri::AppHandle) {
    if !MAIN_WINDOW_SHOWN.swap(true, Ordering::Relaxed) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例:二次启动聚焦已有窗口,而不是再起一个进程+窗口(插件需最先注册)
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        // 记住窗口位置/大小/最大化状态(退出时插件自动落盘)。
        // 只恢复几何信息、不含 VISIBLE——否则恢复时会立刻 show 窗口,破坏上面的延迟显示启动方案
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // 数据目录 + 建库
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join(DB_FILE_NAME);
            let conn = Connection::open(db_path)?;
            db::init_db(&conn).map_err(|e| Box::<dyn std::error::Error>::from(e))?;

            // 一次性迁移旧 data.json(meta 未标记时才读 store、执行迁移)
            let need_migrate = db::get_meta(&conn, db::META_MIGRATED_FROM_STORE)
                .map_err(|e| Box::<dyn std::error::Error>::from(e))?
                .is_none();
            if need_migrate {
                let legacy = db::read_legacy_from_store(app.handle())
                    .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
                db::migrate_from_store(&conn, legacy)
                    .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
            }

            app.manage(db::DbState(Mutex::new(conn)));

            // 兜底:页面加载事件因异常未触发时,超时也强制显示窗口,避免永远不可见
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                show_main_window_once(&handle);
            });
            Ok(())
        })
        // 主页面首次加载完成才显示窗口(见 MAIN_WINDOW_SHOWN);排除 about:blank 初始导航
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
                && payload.url().scheme() != "about"
            {
                show_main_window_once(webview.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            db::list_history,
            db::add_history,
            db::remove_history,
            llm::test_connection,
            llm::generate_report,
            llm::generate_weekly_report,
            collector::collect_conversations,
            collector::collect_conversations_range,
            collector::default_collect_paths,
            export::export_report,
            export::write_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

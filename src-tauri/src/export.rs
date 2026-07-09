use std::fs;
use std::path::PathBuf;

use crate::config::load_config;
use tauri::AppHandle;

/// 导出当日日报。若已配置 exportDir 则直接写入并返回路径；否则返回 None（前端弹窗选择）。
#[tauri::command]
pub async fn export_report(app: AppHandle, content: String) -> Result<Option<String>, String> {
    let cfg = load_config(app)?;
    if cfg.export_dir.trim().is_empty() {
        return Ok(None);
    }
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dir = PathBuf::from(&cfg.export_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{date}.md"));
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// 写入指定路径（前端弹窗选择后调用）。
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| e.to_string())
}

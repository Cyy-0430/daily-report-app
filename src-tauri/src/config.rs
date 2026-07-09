use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "data.json";
const CONFIG_KEY: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub date: String,
    pub title: String,
    pub input: String,
    pub output: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub api_config: ApiConfig,
    #[serde(default)]
    pub prompt_template: String,
    #[serde(default)]
    pub export_dir: String,
    #[serde(default)]
    pub history: Vec<HistoryItem>,
}

#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<AppConfig, String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let cfg = store
        .get(CONFIG_KEY)
        .and_then(|v| serde_json::from_value::<AppConfig>(v).ok())
        .unwrap_or_default();
    Ok(cfg)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&config).map_err(|e| e.to_string())?;
    store.set(CONFIG_KEY, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

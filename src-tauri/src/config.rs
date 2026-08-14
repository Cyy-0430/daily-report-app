use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

use crate::db::{get_config, set_config, DbState};

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
pub struct CollectConfig {
    /// 启用的采集工具 id,默认 "claude-code"、"zcode"、"codex" 与 "opencode"。
    #[serde(default = "default_enabled_tools")]
    pub enabled_tools: Vec<String>,
    /// 仅采集(白名单)的工作目录,空 = 不限。其下子目录一并包含。
    #[serde(default)]
    pub include_paths: Vec<String>,
    /// 排除(黑名单)的工作目录,其下会话一律不采集。排除优先于仅采集。
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    /// 各采集工具的自定义数据源路径(覆盖默认)。键 = 工具 id,值 = 路径串;
    /// 空串或缺失 = 用该工具默认路径。支持 `~` 展开(见 `collector::expand_home`)。
    #[serde(default)]
    pub tool_paths: HashMap<String, String>,
}

/// 旧配置缺失 enabled_tools 时回填默认值。
fn default_enabled_tools() -> Vec<String> {
    // 单一来源:与 collector::all_collectors() 注册集、前端 COLLECT_TOOLS 对齐。
    crate::collector::DEFAULT_TOOL_IDS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl Default for CollectConfig {
    fn default() -> Self {
        Self {
            enabled_tools: default_enabled_tools(),
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
            tool_paths: HashMap::new(),
        }
    }
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

/// 应用配置(历史记录已独立存于 SQLite `history` 表,见 `db` 模块)。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub api_config: ApiConfig,
    #[serde(default)]
    pub prompt_template: String,
    #[serde(default)]
    pub custom_default_template: String,
    /// 周报 map(每日摘要)提示词;空串 → 内嵌默认兜底。
    #[serde(default)]
    pub weekly_map_template: String,
    /// 周报 reduce(整周汇总)提示词;空串 → 内嵌默认兜底。
    #[serde(default)]
    pub weekly_reduce_template: String,
    /// 周报 map 模板的「自定义默认」(设置页「设为默认」写入;「恢复默认」优先于此)。
    #[serde(default)]
    pub weekly_default_map_template: String,
    /// 周报 reduce 模板的「自定义默认」。
    #[serde(default)]
    pub weekly_default_reduce_template: String,
    #[serde(default)]
    pub export_dir: String,
    #[serde(default)]
    pub collect_config: CollectConfig,
    /// 是否在启动时自动检查更新;默认 true。旧配置缺失该字段时回填 true。
    #[serde(default = "default_true")]
    pub auto_check_update: bool,
}

/// `#[serde(default)]` 回退值:布尔默认 true(用于 auto_check_update)。
fn default_true() -> bool {
    true
}

#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<AppConfig, String> {
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    get_config(&conn)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    set_config(&conn, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_auto_check_update_defaults_true() {
        // 旧配置 JSON 缺 autoCheckUpdate 字段时,反序列化必须回退为 true(无损升级)。
        let legacy = r#"{
            "apiConfig": { "baseUrl": "", "apiKey": "", "model": "" },
            "collectConfig": {
                "enabledTools": ["claude-code"],
                "includePaths": [],
                "excludePaths": [],
                "toolPaths": {}
            }
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).expect("parse legacy config");
        assert!(cfg.auto_check_update, "auto_check_update 应默认 true");
    }

    #[test]
    fn auto_check_update_round_trips() {
        let mut cfg = AppConfig::default();
        cfg.auto_check_update = false;
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert!(!back.auto_check_update);
        // 序列化键须为 camelCase,与前端对齐。
        assert!(json.contains("\"autoCheckUpdate\":false"));
    }
}

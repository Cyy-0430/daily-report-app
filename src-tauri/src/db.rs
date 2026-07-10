//! SQLite 数据持久层。
//!
//! 单连接(`Mutex<Connection>`)经 Tauri State 共享,串行访问满足单用户桌面应用。
//! 命令层(`#[tauri::command]`)从 `DbState` 取连接,委托给接受 `&Connection` 的
//! DAO 函数 —— DAO 不依赖 Tauri,可用 `Connection::open_in_memory()` 做单元测试。

use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::config::{ApiConfig, AppConfig, CollectConfig, HistoryItem};

/// 应用数据库连接状态(单连接,Mutex 串行保护)。
pub struct DbState(pub Mutex<Connection>);

// ===========================================================================
// Schema 与初始化
// ===========================================================================

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS history (
  id          TEXT PRIMARY KEY,
  date        TEXT NOT NULL,
  title       TEXT NOT NULL DEFAULT '',
  input       TEXT NOT NULL DEFAULT '',
  output      TEXT NOT NULL DEFAULT '',
  created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_history_date       ON history(date);
CREATE INDEX IF NOT EXISTS idx_history_created_at ON history(created_at DESC);

CREATE TABLE IF NOT EXISTS config (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

/// 初始化 pragma + schema + `schema_version`。幂等(`IF NOT EXISTS`)。
///
/// WAL 对文件库提升崩溃恢复;对内存库(`:memory:`)无意义但被 SQLite 静默忽略,
/// 故忽略其返回值以保证测试与生产行为一致。
pub fn init_db(conn: &Connection) -> Result<(), String> {
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    conn.execute_batch(SCHEMA_SQL).map_err(|e| e.to_string())?;
    set_meta(conn, "schema_version", "1")?;
    Ok(())
}

// ===========================================================================
// meta
// ===========================================================================

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES(?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM meta WHERE key=?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

// ===========================================================================
// config KV DAO(value 存 JSON 序列化值,新增配置项无需 ALTER TABLE)
// ===========================================================================

/// 读取全部配置,组装 `AppConfig`(缺失 key 用 default)。
pub fn get_config(conn: &Connection) -> Result<AppConfig, String> {
    let mut cfg = AppConfig::default();
    if let Some(v) = get_kv(conn, "api_config")? {
        cfg.api_config = serde_json::from_str(&v).map_err(|e| e.to_string())?;
    }
    if let Some(v) = get_kv(conn, "prompt_template")? {
        cfg.prompt_template = serde_json::from_str(&v).map_err(|e| e.to_string())?;
    }
    if let Some(v) = get_kv(conn, "custom_default_template")? {
        cfg.custom_default_template = serde_json::from_str(&v).map_err(|e| e.to_string())?;
    }
    if let Some(v) = get_kv(conn, "export_dir")? {
        cfg.export_dir = serde_json::from_str(&v).map_err(|e| e.to_string())?;
    }
    if let Some(v) = get_kv(conn, "collect_config")? {
        cfg.collect_config = serde_json::from_str(&v).map_err(|e| e.to_string())?;
    }
    Ok(cfg)
}

fn get_kv(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM config WHERE key=?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn config_pairs(cfg: &AppConfig) -> Result<Vec<(&'static str, String)>, String> {
    Ok(vec![
        ("api_config", serde_json::to_string(&cfg.api_config).map_err(|e| e.to_string())?),
        ("prompt_template", serde_json::to_string(&cfg.prompt_template).map_err(|e| e.to_string())?),
        ("custom_default_template", serde_json::to_string(&cfg.custom_default_template).map_err(|e| e.to_string())?),
        ("export_dir", serde_json::to_string(&cfg.export_dir).map_err(|e| e.to_string())?),
        ("collect_config", serde_json::to_string(&cfg.collect_config).map_err(|e| e.to_string())?),
    ])
}

/// upsert 配置(无独立事务,调用方负责事务边界)。
fn upsert_config(conn: &Connection, cfg: &AppConfig) -> Result<(), String> {
    let mut stmt = conn
        .prepare("INSERT OR REPLACE INTO config(key, value) VALUES(?1, ?2)")
        .map_err(|e| e.to_string())?;
    for (key, value) in config_pairs(cfg)? {
        stmt.execute(params![key, value]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 保存配置(`unchecked_transaction` 包裹,原子完成)。
pub fn set_config(conn: &Connection, cfg: &AppConfig) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    upsert_config(&tx, cfg)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// ===========================================================================
// history DAO
// ===========================================================================

/// 全部历史,按 `created_at DESC`(等价旧 `insert(0)` 的「最新在前」)。
pub fn fetch_history(conn: &Connection) -> Result<Vec<HistoryItem>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, date, title, input, output, created_at \
             FROM history ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                date: row.get(1)?,
                title: row.get(2)?,
                input: row.get(3)?,
                output: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

/// 新增/覆盖一条历史(`INSERT OR REPLACE`,支持未来导入去重)。
pub fn insert_history(conn: &Connection, item: &HistoryItem) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO history(id, date, title, input, output, created_at) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![item.id, item.date, item.title, item.input, item.output, item.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 删除一条历史。
pub fn delete_history(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM history WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ===========================================================================
// 迁移:旧 tauri-plugin-store(data.json)→ SQLite
// ===========================================================================

/// 旧 `data.json` 中的配置(含历史),仅用于一次性迁移读取。
/// 运行时配置(`AppConfig`)已不含 `history`,故迁移源需独立结构。
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyAppConfig {
    #[serde(default)]
    api_config: ApiConfig,
    #[serde(default)]
    prompt_template: String,
    #[serde(default)]
    custom_default_template: String,
    #[serde(default)]
    export_dir: String,
    #[serde(default)]
    collect_config: CollectConfig,
    #[serde(default)]
    history: Vec<HistoryItem>,
}

/// 从旧 tauri-plugin-store 读取 legacy 配置(含历史)。无数据返回 `None`。
pub fn read_legacy_from_store(app: &AppHandle) -> Result<Option<LegacyAppConfig>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store("data.json").map_err(|e| e.to_string())?;
    match store.get("config") {
        Some(v) => {
            let leg: LegacyAppConfig = serde_json::from_value(v).map_err(|e| e.to_string())?;
            Ok(Some(leg))
        }
        None => Ok(None),
    }
}

/// 将 legacy 数据迁移进 SQLite。幂等:`meta.migrated_from_store` 已置位则跳过。
/// 事务保护:历史批量插入 + 配置 upsert + 标记置位原子完成;**不删旧 `data.json`**。
/// 返回是否实际执行了迁移。
pub fn migrate_from_store(
    conn: &Connection,
    legacy: Option<LegacyAppConfig>,
) -> Result<bool, String> {
    if get_meta(conn, "migrated_from_store")?.is_some() {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    if let Some(leg) = legacy {
        // 历史:`INSERT OR IGNORE` 保证重复迁移幂等(同 id 不覆盖本地后续修改)。
        for item in &leg.history {
            tx.execute(
                "INSERT OR IGNORE INTO history(id, date, title, input, output, created_at) \
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![item.id, item.date, item.title, item.input, item.output, item.created_at],
            )
            .map_err(|e| e.to_string())?;
        }
        // 配置
        let cfg = AppConfig {
            api_config: leg.api_config,
            prompt_template: leg.prompt_template,
            custom_default_template: leg.custom_default_template,
            export_dir: leg.export_dir,
            collect_config: leg.collect_config,
        };
        upsert_config(&tx, &cfg)?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES('migrated_from_store', '1')",
        [],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

// ===========================================================================
// Tauri 命令(委托给上方 DAO)
// ===========================================================================

#[tauri::command]
pub fn list_history(app: AppHandle) -> Result<Vec<HistoryItem>, String> {
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    fetch_history(&conn)
}

#[tauri::command]
pub fn add_history(app: AppHandle, item: HistoryItem) -> Result<(), String> {
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    insert_history(&conn, &item)
}

#[tauri::command]
pub fn remove_history(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    delete_history(&conn, &id)
}

// ===========================================================================
// 单元测试(内存库,无 Tauri 依赖)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ApiConfig, CollectConfig};
    use rusqlite::Connection;

    /// 新建内存库并初始化 schema。
    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    fn sample_item(id: &str, created_at: i64) -> HistoryItem {
        HistoryItem {
            id: id.into(),
            date: "2026-07-10".into(),
            title: format!("标题-{id}"),
            input: format!("输入-{id}"),
            output: format!("输出-{id}"),
            created_at,
        }
    }

    fn sample_config() -> AppConfig {
        AppConfig {
            api_config: ApiConfig {
                base_url: "https://api.example.com/v1".into(),
                api_key: "sk-xxx".into(),
                model: "gpt-demo".into(),
            },
            prompt_template: "模板{{input}}".into(),
            custom_default_template: "默认".into(),
            export_dir: "D:\\export".into(),
            collect_config: CollectConfig {
                enabled_tools: vec!["claude-code".into()],
                include_paths: vec!["D:\\work".into()],
                exclude_paths: vec!["D:\\secret".into()],
            },
        }
    }

    fn empty_legacy() -> LegacyAppConfig {
        LegacyAppConfig {
            api_config: ApiConfig::default(),
            prompt_template: String::new(),
            custom_default_template: String::new(),
            export_dir: String::new(),
            collect_config: CollectConfig::default(),
            history: Vec::new(),
        }
    }

    #[test]
    fn init_db_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        init_db(&conn).unwrap(); // 再次执行不报错
        assert_eq!(get_meta(&conn, "schema_version").unwrap(), Some("1".into()));
    }

    #[test]
    fn config_roundtrip() {
        let conn = mem_db();
        let cfg = sample_config();
        set_config(&conn, &cfg).unwrap();
        let got = get_config(&conn).unwrap();
        assert_eq!(got.api_config.base_url, cfg.api_config.base_url);
        assert_eq!(got.api_config.api_key, cfg.api_config.api_key);
        assert_eq!(got.api_config.model, cfg.api_config.model);
        assert_eq!(got.prompt_template, cfg.prompt_template);
        assert_eq!(got.custom_default_template, cfg.custom_default_template);
        assert_eq!(got.export_dir, cfg.export_dir);
        assert_eq!(got.collect_config.enabled_tools, cfg.collect_config.enabled_tools);
        assert_eq!(got.collect_config.include_paths, cfg.collect_config.include_paths);
        assert_eq!(got.collect_config.exclude_paths, cfg.collect_config.exclude_paths);
    }

    #[test]
    fn config_empty_db_returns_default() {
        let conn = mem_db();
        let got = get_config(&conn).unwrap();
        assert_eq!(got.api_config.base_url, "");
        assert_eq!(got.export_dir, "");
        // default_enabled_tools 回填 claude-code
        assert_eq!(got.collect_config.enabled_tools, vec!["claude-code".to_string()]);
    }

    #[test]
    fn config_partial_update() {
        let conn = mem_db();
        let mut cfg = sample_config();
        set_config(&conn, &cfg).unwrap();
        cfg.export_dir = "D:\\new".into();
        set_config(&conn, &cfg).unwrap();
        let got = get_config(&conn).unwrap();
        assert_eq!(got.export_dir, "D:\\new");
        assert_eq!(got.api_config.model, "gpt-demo"); // 其余字段不变
    }

    #[test]
    fn history_order_desc_by_created_at() {
        let conn = mem_db();
        insert_history(&conn, &sample_item("a", 1000)).unwrap();
        insert_history(&conn, &sample_item("b", 3000)).unwrap();
        insert_history(&conn, &sample_item("c", 2000)).unwrap();
        let list = fetch_history(&conn).unwrap();
        let ids: Vec<_> = list.into_iter().map(|h| h.id).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn history_delete() {
        let conn = mem_db();
        insert_history(&conn, &sample_item("a", 1)).unwrap();
        insert_history(&conn, &sample_item("b", 2)).unwrap();
        delete_history(&conn, "a").unwrap();
        let list = fetch_history(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "b");
    }

    #[test]
    fn history_insert_or_replace_overwrites_same_id() {
        let conn = mem_db();
        insert_history(&conn, &sample_item("a", 1)).unwrap();
        let mut updated = sample_item("a", 1);
        updated.title = "新标题".into();
        insert_history(&conn, &updated).unwrap();
        let list = fetch_history(&conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "新标题");
    }

    #[test]
    fn history_special_chars_roundtrip() {
        let conn = mem_db();
        let item = HistoryItem {
            id: "sp".into(),
            date: "2026-07-10".into(),
            title: "引号\"和\\斜杠".into(),
            input: "emoji 🎉 中文\n换行\t制表".into(),
            output: "很长的文本".repeat(1000),
            created_at: 42,
        };
        insert_history(&conn, &item).unwrap();
        let got = fetch_history(&conn).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title, item.title);
        assert_eq!(got[0].input, item.input);
        assert_eq!(got[0].output, item.output);
    }

    #[test]
    fn history_empty_strings() {
        let conn = mem_db();
        let item = HistoryItem {
            id: "empty".into(),
            date: String::new(),
            title: String::new(),
            input: String::new(),
            output: String::new(),
            created_at: 0,
        };
        insert_history(&conn, &item).unwrap();
        let got = fetch_history(&conn).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, "empty");
    }

    #[test]
    fn migrate_imports_history_and_config() -> Result<(), String> {
        let conn = mem_db();
        let mut legacy = empty_legacy();
        legacy.api_config = ApiConfig {
            base_url: "u".into(),
            api_key: "k".into(),
            model: "m".into(),
        };
        legacy.prompt_template = "t".into();
        legacy.export_dir = "e".into();
        legacy.history = vec![sample_item("x", 5), sample_item("y", 9)];

        assert_eq!(migrate_from_store(&conn, Some(legacy.clone()))?, true);

        // 历史:created_at DESC → y 在前
        let list = fetch_history(&conn)?;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "y");
        assert_eq!(list[1].id, "x");
        // 配置
        let cfg = get_config(&conn)?;
        assert_eq!(cfg.api_config.base_url, "u");
        assert_eq!(cfg.prompt_template, "t");
        assert_eq!(cfg.export_dir, "e");
        // 标记置位
        assert_eq!(get_meta(&conn, "migrated_from_store")?, Some("1".into()));
        Ok(())
    }

    #[test]
    fn migrate_is_idempotent() -> Result<(), String> {
        let conn = mem_db();
        let mut legacy = empty_legacy();
        legacy.history = vec![sample_item("x", 5)];

        assert_eq!(migrate_from_store(&conn, Some(legacy.clone()))?, true);
        // 第二次:meta 已标记,跳过,不重复插入
        assert_eq!(migrate_from_store(&conn, Some(legacy))?, false);
        assert_eq!(fetch_history(&conn)?.len(), 1);
        Ok(())
    }

    #[test]
    fn migrate_none_legacy_marks_without_data() -> Result<(), String> {
        let conn = mem_db();
        assert_eq!(migrate_from_store(&conn, None)?, true);
        assert_eq!(fetch_history(&conn)?.len(), 0);
        assert_eq!(get_config(&conn)?.export_dir, ""); // 仍为默认
        assert_eq!(get_meta(&conn, "migrated_from_store")?, Some("1".into()));
        Ok(())
    }
}

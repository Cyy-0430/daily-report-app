# SQLite 持久层契约 (Storage Spec)

> 后端 `src-tauri/src/db.rs` + `config.rs` 的可执行契约。覆盖 SQLite schema、配置 KV 模型、
> 历史记录 CRUD、旧 `data.json` 迁移,以及配置/历史解耦后的跨层命令契约。

## Scenario: 数据持久层(SQLite)

### 1. Scope / Trigger

触发 code-spec 深度的原因:
- 新增/变更 Tauri 命令签名(`list_history`/`add_history`/`remove_history` 新增;`generate_report` 返回类型变为 `HistoryItem`)。
- 跨层契约变更:`AppConfig` **移除 `history` 字段**,历史独立为表 + 独立命令。
- 数据库 schema 与迁移变更(`history`/`config`/`meta` 三表;旧 store → SQLite 一次性迁移)。
- 基础设施集成:存储引擎从 `tauri-plugin-store`(JSON 文件)切换为 `rusqlite`(SQLite)。

任何修改 db.rs、配置/历史命令、schema、或迁移逻辑的工作,都必须遵守本契约。

### 2. Signatures

**存储引擎**(`db.rs`):
```rust
// 单连接,Mutex 串行保护,经 Tauri State 共享(`app.manage(DbState(Mutex::new(conn)))`)
pub struct DbState(pub Mutex<Connection>);

// 文件:<app_data_dir>/daily_report.db  (app.path().app_data_dir())
// 依赖:rusqlite = { version = "0.40", features = ["bundled"] }  // bundled 编译 SQLite 源码,无系统依赖
```

**Schema**(`init_db`,全 `IF NOT EXISTS` 幂等):
```sql
CREATE TABLE history (
  id          TEXT PRIMARY KEY,          -- UUID v4,跨设备去重主键
  date        TEXT NOT NULL,             -- 'YYYY-MM-DD'
  title       TEXT NOT NULL DEFAULT '',
  input       TEXT NOT NULL DEFAULT '',
  output      TEXT NOT NULL DEFAULT '',
  created_at  INTEGER NOT NULL           -- 秒级时间戳(chrono Local::timestamp())
);
CREATE INDEX idx_history_date       ON history(date);
CREATE INDEX idx_history_created_at ON history(created_at DESC);

CREATE TABLE config ( key TEXT PRIMARY KEY, value TEXT NOT NULL );  -- value 存 JSON 序列化值
CREATE TABLE meta   ( key TEXT PRIMARY KEY, value TEXT NOT NULL );  -- schema_version / migrated_from_store
```

**DAO(纯函数,接受 `&Connection`,不依赖 Tauri → 可用 `open_in_memory()` 测试)**:
```rust
pub fn init_db(conn: &Connection) -> Result<(), String>
pub fn get_config(conn: &Connection) -> Result<AppConfig, String>
pub fn set_config(conn: &Connection, cfg: &AppConfig) -> Result<(), String>          // unchecked_transaction 包裹
pub fn fetch_history(conn: &Connection) -> Result<Vec<HistoryItem>, String>          // ORDER BY created_at DESC
pub fn insert_history(conn: &Connection, item: &HistoryItem) -> Result<(), String>   // INSERT OR REPLACE
pub fn delete_history(conn: &Connection, id: &str) -> Result<(), String>
pub fn migrate_from_store(conn: &Connection, legacy: Option<LegacyAppConfig>) -> Result<bool, String>  // 返回是否实际迁移
pub fn read_legacy_from_store(app: &AppHandle) -> Result<Option<LegacyAppConfig>, String>
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, String>
```

**Tauri 命令**(委托给 DAO,从 `DbState` 取连接):
```rust
#[tauri::command] fn load_config(app) -> Result<AppConfig, String>           // 签名不变,内部走 KV 表
#[tauri::command] fn save_config(app, config: AppConfig) -> Result<(), String> // 签名不变,内部走 KV 表
#[tauri::command] fn list_history(app) -> Result<Vec<HistoryItem>, String>
#[tauri::command] fn add_history(app, item: HistoryItem) -> Result<(), String>
#[tauri::command] fn remove_history(app, id: String) -> Result<(), String>
#[tauri::command] async fn generate_report(app, input, conversations, on_event: Channel<StreamChunk>)
    -> Result<HistoryItem, String>   // 返回已保存的 item,前端免重新拉取即可同步 store
```

### 3. Contracts

#### 3a. 配置 KV 模型
- `config` 表为 KV:`key` ∈ {`api_config`,`prompt_template`,`custom_default_template`,`export_dir`,`collect_config`}。
- **`value` 一律存 JSON 序列化值**(`serde_json::to_string`):string 字段存带引号 JSON 串,struct 存 JSON object。读写统一走 serde,新增配置项无需 `ALTER TABLE`。
- `get_config`:缺失 key 回填 `AppConfig::default()`(`#[serde(default)]`);`CollectConfig` 默认 `enabled_tools=["claude-code"]`。

#### 3b. 历史记录跨层契约(硬契约)
- **`AppConfig` 不含 `history`**。历史仅经 `list_history`/`add_history`/`remove_history` 访问,**不得再走 `load_config`→改→`save_config` 全量读写**。
- `HistoryItem { id, date, title, input, output, createdAt }`,`createdAt` 为 **i64/number 秒级**(与 `chrono::Local::now().timestamp()` 一致);**禁止改为毫秒**,否则迁移旧数据 round-trip 错乱、排序失序。
- Rust struct 必须 `#[serde(rename_all = "camelCase")]`,与 TS `HistoryItem` interface 一一对应(继承 collector-spec 3c)。
- `fetch_history` 固定 `ORDER BY created_at DESC`(等价旧 `history.insert(0, item)` 的「最新在前」)。

#### 3c. 迁移契约(旧 `data.json` → SQLite)
- **触发点**:`lib.rs` 的 `setup` 钩子。流程:`init_db` → 检查 `meta.migrated_from_store` → 若无,`read_legacy_from_store` → `migrate_from_store`。
- **迁移源结构**:`LegacyAppConfig`(含 `history`),仅迁移用;运行时 `AppConfig` 无 `history`。两者字段除 `history` 外一一对应。
- **幂等**:`migrate_from_store` 首行检查 `meta.migrated_from_store`,已置位则返回 `false` 跳过;历史用 `INSERT OR IGNORE`(同 id 不覆盖本地后续修改)。
- **原子**:`unchecked_transaction` 包裹「历史批量插入 + 配置 upsert + meta 置位」;失败回滚。
- **不删 `data.json`**:迁移源保留作回退;`tauri-plugin-store` 依赖本轮保留(后续清理任务再移除)。

#### 3d. 数据目录
- `<app_data_dir>`(Tauri 2:`app.path().app_data_dir()`,Windows 为 `%APPDATA%\<identifier>`,本项目 identifier=`com.cyy.dailyreport`)。
- 同目录文件:`data.json`(旧,迁移源)、`daily_report.db` + `.db-wal` + `.db-shm`(WAL 模式一组,备份/删除需一起)。

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| `app_data_dir` 不可创建 / db 不可打开 | `setup` 返回 `Err`,应用启动失败 |
| `init_db` 重复执行 | `IF NOT EXISTS` 幂等,不报错 |
| 旧 `data.json` 不存在(全新环境) | `read_legacy_from_store` 返回 `None`;`migrate(None)` 空迁移 + 置 meta 标记 |
| 旧 `data.json` `config` key 缺失 | 同上(`None`) |
| 旧 `data.json` 损坏 / 反序列化失败 | `from_value` 返回 `Err`;`data.json` 未删,可手动恢复后重试 |
| 重复启动(已迁移) | `meta.migrated_from_store` 已置位 → 跳过,历史不重复 |
| `get_config` 缺失某 key | 该字段回填 default,其余保留 |
| 历史 `add` 同 id | `INSERT OR REPLACE` 覆盖 |
| `app.state::<DbState>()` 取连接 | 必须 `let state = ...; let conn = state.0.lock()...;`(State 临时值生命周期,见 7) |

### 5. Good / Base / Bad Cases

- **Good**:旧 `data.json` 含 22 条历史 + 完整配置 → 首启全部迁入,`meta` 置位;再启跳过,数据稳定。
- **Base**:全新环境(无 `data.json`)→ 空库,各页面正常,历史为空。
- **Bad(反例,不得实现)**:
  - 迁移成功后删除 `data.json` → 丧失回退能力。
  - 历史增删走 `load_config`→改→`save_config` 全量读写(见 7)。
  - `created_at` 改用毫秒 → 旧数据排序/显示错乱。

### 6. Tests Required

`db.rs` 的 `#[cfg(test)]` 必须覆盖(用 `Connection::open_in_memory()`,断言点):

- `init_db` 幂等:重复执行不报错,`schema_version='1'`。
- config:`set`→`get` round-trip 全字段等价;空库返回 default(含 `enabled_tools=["claude-code"]`);部分字段更新不影响其余。
- history:`add` 多条后 `fetch` 为 `created_at DESC`;`delete` 仅删目标;同 id `INSERT OR REPLACE` 覆盖;特殊字符(引号/`\`/emoji/换行/长文本)round-trip;空串边界。
- 迁移 `migrate_from_store`:给定 legacy(含 history)→ `fetch` 内容一致 + 配置一致 + `migrated_from_store='1'`;**重复调用返回 `false` 且不重复插入**;`None` legacy 不报错且置标记。

前端(`vitest` + jsdom):
- `renderMarkdown`:标题/有序列表 `<ol>`/无序列表 `<ul>`/加粗/XSS 清理 `<script>`/中文特殊字符/空串。

### 7. Wrong vs Correct

#### Wrong — 取 State 连接的临时值生命周期
```rust
// ❌ app.state() 返回临时 State,语句末释放,而 MutexGuard 仍借用 → E0716 编译失败
let conn = app.state::<DbState>().0.lock().map_err(|e| e.to_string())?;
fetch_history(&conn)
```

#### Correct — State 绑定到 let
```rust
// ✅ State 活过 guard 的使用
let state = app.state::<DbState>();
let conn = state.0.lock().map_err(|e| e.to_string())?;
fetch_history(&conn)
```

#### Wrong — 历史增删走全量配置读写
```rust
// ❌ O(n) 全量序列化/反序列化,配置与历史耦合,且 history 已不在 AppConfig
let mut cfg = load_config(app.clone())?;
cfg.history.retain(|h| h.id != id);   // 编译错:AppConfig 无 history
save_config(app, cfg)?;
```

#### Correct — 细粒度历史命令
```rust
// ✅ O(1) 单行 DELETE,配置不受影响
delete_history(&conn, &id)?;
// 前端:await removeHistory(id); history.update(h => h.filter(x => x.id !== id));
```

#### Wrong — 用 happy-dom 跑 DOMPurify 测试
```text
// ❌ happy-dom HTML 解析不完整,DOMPurify 在其下会吞掉 <h1>/<ol>/<ul> 等块级标签
//    renderMarkdown("# x") → "x"(h1 丢失)→ 假失败,且与生产 webview 行为不一致
environment: "happy-dom"
```

#### Correct — 用 jsdom
```text
// ✅ jsdom 是 DOMPurify 官方支持的成熟 DOM 实现,输出与生产 Chromium webview 一致
environment: "jsdom"
```

---

## 关联
- 任务来源:`.trellis/tasks/07-10-sqlite-storage/`(prd/design/implement)。
- 采集命令契约(不受本持久层重构影响)见 `collector-spec.md`。
- 跨层 serde camelCase 约定继承 `collector-spec.md` 3c。

# Design — 引入 SQLite 重构数据持久层

> 配套 `prd.md`。本轮范围:SQLite 数据层 + 旧 `data.json` 迁移 + 现有功能等价回归 + 数据层单元测试。导入导出/历史搜索仅预留 schema。

## 1. 架构与边界

### 新增/改动模块

| 文件 | 角色 | 改动 |
|---|---|---|
| `src-tauri/src/db.rs` | **新增** 数据层 | 连接管理、schema、config/history CRUD、迁移核心逻辑 |
| `src-tauri/src/lib.rs` | 入口 | 注册 `DbState`、加 `setup` 钩子做建库+迁移 |
| `src-tauri/src/config.rs` | 配置命令 | `load_config`/`save_config` 实现从 store 切到 KV 表;`AppConfig` **移除 `history` 字段** |
| `src-tauri/src/llm.rs` | 生成日报 | 流式完成后改调 `db::add_history`,不再 `load→insert→save` |
| `src-tauri/Cargo.toml` | 依赖 | 加 `rusqlite = { version = "0.40", features = ["bundled"] }` |
| `src/lib/bindings.ts` | 前端契约 | `AppConfig` 去 `history`;新增 `list_history`/`add_history`/`remove_history` |
| `src/lib/store.ts` | 全局 store | 新增独立 `history` store;`initConfig` 同时加载历史 |
| `src/routes/history/+page.svelte` | 历史页 | 列表读 history store,删除调 `remove_history(id)` |
| `src/routes/settings/+page.svelte` | 设置页 | `save_config` 不再带 history(类型已去);其余不变 |

### 数据库定位与连接

- 数据库文件:`<app_data_dir>/daily_report.db`(`app.path().app_data_dir()`,与现 `data.json` 同目录)。
- 连接:`std::sync::Mutex<rusqlite::Connection>`,封装为 `DbState`,通过 `app.manage()` 注册为 Tauri State。
- 命令内:`state.0.lock()?.execute/query(...)`。单连接 + Mutex 满足当前并发量(单用户桌面应用)。
- 连接初始化时执行:`PRAGMA journal_mode=WAL;`(更稳的崩溃恢复,无害)。

### 依赖选择

- **rusqlite `bundled`**:把 SQLite 源码编进二进制,无系统 SQLite 依赖。Windows 上尤其关键(避免链接系统 SQLite 的坑)。代价:首次编译稍慢、二进制约 +0.5MB。
- **不引入** `sqlx`(需离线 schema,对单表 CRUD 过重)、`tauri-plugin-sql`(前端写 SQL,破坏命令封装)。
- **保留** `tauri-plugin-store` 依赖与 `data.json` 作为迁移源;本轮不移除,降低风险(留作后续清理)。

## 2. Schema

```sql
-- 历史(核心)
CREATE TABLE IF NOT EXISTS history (
  id          TEXT PRIMARY KEY,          -- UUID v4,跨设备去重主键(为未来导入导出预留)
  date        TEXT NOT NULL,             -- 'YYYY-MM-DD'
  title       TEXT NOT NULL DEFAULT '',
  input       TEXT NOT NULL DEFAULT '',
  output      TEXT NOT NULL DEFAULT '',
  created_at  INTEGER NOT NULL           -- 秒级时间戳(与 chrono Local::timestamp() 一致,迁移旧数据保持原值)
);
CREATE INDEX IF NOT EXISTS idx_history_date       ON history(date);
CREATE INDEX IF NOT EXISTS idx_history_created_at ON history(created_at DESC);

-- 配置(KV,每项 value 存 JSON 字符串;新增配置项无需 ALTER TABLE)
CREATE TABLE IF NOT EXISTS config (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- 元数据(schema 版本、迁移标记,为未来 schema 演进/导入导出预留)
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- 初始: schema_version='1',migrated_from_store 在迁移完成后置 '1'
```

`config` 的 key:`api_config` / `prompt_template` / `custom_default_template` / `export_dir` / `collect_config`(各存一行,value 为 JSON)。

## 3. 命令契约(对外 API)

**保留(实现切换,签名不变):**
- `load_config(app) -> AppConfig` —— 从 `config` 表组装(缺失 key 用 default)。**不再含 history**。
- `save_config(app, AppConfig)` —— 事务内 upsert 各 key 到 `config` 表。

**新增(history 细粒度操作,取代全量读写):**
- `list_history(app, limit: Option<i64>, offset: Option<i64>, keyword: Option<String>) -> Vec<HistoryItem>` —— 按 `created_at DESC`;本轮前端取全部(等价旧行为),`limit/offset/keyword` 为未来搜索分页预留。
- `add_history(app, HistoryItem)` —— `INSERT OR REPLACE`。
- `remove_history(app, id: String)` —— `DELETE WHERE id=?`。

> **关键解耦**:旧流程中 `history/+page.svelte` 删除 = `load→filter→save`、`llm.rs` 追加 = `load→insert→save`,均为全量读写。重构后分别走 `remove_history` / `add_history`,高效且互不影响配置。

## 4. 数据流

### 启动(setup 钩子,lib.rs)
1. `app_data_dir = app.path().app_data_dir()?; create_dir_all`。
2. `conn = Connection::open(app_data_dir.join("daily_report.db"))?`。
3. `PRAGMA journal_mode=WAL` + 执行建表 SQL + 写 `meta.schema_version='1'`(IF NOT EXISTS 幂等)。
4. 迁移(见下)。
5. `app.manage(DbState(Mutex::new(conn)))`。

### 迁移(核心逻辑抽成可测纯函数)
```rust
// db.rs — 纯逻辑,便于单元测试(传入内存 conn + 构造的旧数据)
pub fn migrate_from_store(conn: &Connection, old: Option<AppConfig>) -> Result<()>
// 命令/setup 里:从 store 读旧 AppConfig,再调上面函数
```
- 读 `meta.migrated_from_store`:已为 `'1'` → 跳过。
- 首次:从 `tauri-plugin-store` 的 `data.json` 读 `config` key → 反序列化 `AppConfig`(含 `history`)。
  - 有数据:`history` 批量 `INSERT OR IGNORE` 进 `history` 表;各配置字段 upsert 进 `config` 表。**整体在一个事务内**。
  - 无数据(全新环境):空库直接工作。
- 置 `meta.migrated_from_store='1'`。
- **不删 `data.json`**(保留作回退)。

### 运行期
- 前端 `initConfig`:`load_config` 填充 config store + `list_history` 填充 history store。
- 生成日报:`llm.rs` 完成后 `add_history` + 同步更新前端 history store。
- 历史页删除:`remove_history` + 前端 store 过滤。
- 设置页保存:`save_config` 仅写配置表(历史不受影响)。

## 5. 兼容性与迁移

- **一次性、幂等、事务保护**:`meta.migrated_from_store` 保证只迁一次;迁移失败事务回滚,旧 `data.json` 完好。
- **类型破坏性变更**:`AppConfig` 移除 `history` 字段。影响面仅限前端 `bindings.ts` 类型与 3 个消费点,均在改动清单内,可控。
- **回滚**:迁移不删 `data.json`;若新代码出问题,回退代码版本后旧 `data.json` 仍在,可继续用。db 文件独立,删除即回到空库并可重迁。

## 6. 测试策略(对应 prd 决策 3)

### Rust 单元测试(`cargo test`,核心)
用 `Connection::open_in_memory()` 做隔离、快速的测试库。覆盖:
- **schema**:建表 + 索引存在;重复执行幂等。
- **config CRUD**:`save_config` 后 `load_config` 等价;缺失 key 回退 default;部分字段更新。
- **history CRUD**:`add`→`list`(按 `created_at DESC` 顺序);`remove`;重复 id(`INSERT OR REPLACE`)。
- **迁移**:`migrate_from_store` 给定旧 `AppConfig`(含 history)→ 验证 `history`/`config` 表内容一致 + `meta` 标记置位;**重复调用幂等**(不重复插入);空旧数据不报错;特殊字符(引号、emoji、长文本、空串)正确落库。
- 迁移核心抽成接受 `&Connection + AppConfig` 的纯函数,正是为可测性服务。

### 前端纯逻辑测试(引入 vitest)
- 加 `vitest` + `jsdom` dev 依赖。
  - **jsdom 而非 happy-dom**:DOMPurify 在 happy-dom 下会吞掉 `<h1>`/`<ol>`/`<ul>` 等块级标签(happy-dom 解析不完整),造成假失败;jsdom 是 DOMPurify 官方支持的成熟实现,输出与生产 webview 一致。
- 测 `src/lib/markdown.ts` `renderMarkdown`:标题、有序/无序列表结构、加粗、XSS 清理 `<script>`、中文/特殊字符、空串不抛错。
- 测 `src/lib/template.ts` 常量非空 + 占位符。
- 不引入组件测试框架。

### 手动功能回归清单(写入 `implement.md`)
逐项验证用户可见功能(见 implement.md「回归清单」),覆盖涉及网络/LLM/文件、单测无法覆盖的部分。

## 7. 主要权衡

| 决策 | 选择 | 理由 / 代价 |
|---|---|---|
| SQLite 库 | rusqlite + bundled | 同步、轻、契合命令封装;Windows 打包无系统依赖。代价:二进制 +0.5MB、首编稍慢 |
| AppConfig 去 history | 移除 | 真正解耦;代价:前端类型破坏性变更(面小可控) |
| store 依赖 | 本轮保留 | 作迁移源,降低风险;后续任务清理 |
| 连接 | 单 `Mutex<Connection>` | 满足单用户桌面并发;简单可靠 |
| 测试边界 | 数据层单测 + 纯逻辑 + 手动清单 | 务实;网络/LLM/文件功能靠手动回归 |

## 8. 为未来预留(本轮不实现)

- `meta.schema_version`:未来 schema 变更走版本化迁移。
- `history.id` UUID:跨设备导入合并的去重键。
- `list_history(limit, offset, keyword)` + 索引:搜索/分页的现成接口。
- `config` KV:导入导出配置时 `SELECT *` 即可。

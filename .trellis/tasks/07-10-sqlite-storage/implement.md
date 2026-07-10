# Implement — 引入 SQLite 重构数据持久层

> 配套 `prd.md` / `design.md`。按顺序执行,每阶段用对应验证命令确认后再进下一阶段。**用户数据迁移是最高风险点**,严格遵守回滚约束(不删 `data.json`)。

## 阶段 A — 后端数据层

- [ ] A1. `src-tauri/Cargo.toml` 加 `rusqlite = { version = "0.40", features = ["bundled"] }`
- [ ] A2. 新建 `src-tauri/src/db.rs`:
  - `DbState(pub Mutex<Connection>)`
  - `init_db(conn)`:WAL pragma + 建表(history/config/meta)+ 索引 + `meta.schema_version='1'`(全 `IF NOT EXISTS`,幂等)
  - config DAO:`get_config(conn) -> AppConfig`、`set_config(conn, &AppConfig)`(事务内 upsert 各 key)
  - history DAO:`list_history(conn, limit, offset, keyword)`、`add_history(conn, &HistoryItem)`(`INSERT OR REPLACE`)、`remove_history(conn, id)`
  - `migrate_from_store(conn, old: Option<AppConfig>) -> Result<()>`(纯逻辑:批量 insert history + upsert config + 置 `migrated_from_store='1'`,事务包裹,`INSERT OR IGNORE` 保证幂等)
- [ ] A3. `config.rs`:`AppConfig` **移除 `history` 字段**;`load_config`/`save_config` 改用 `db::get_config`/`set_config`(经 `DbState`)
- [ ] A4. `llm.rs`:生成完成处(`llm.rs:175-185`)改调 `db::add_history`,删除 `load→insert→save` 全量写
- [ ] A5. `lib.rs`:
  - 注册新命令 `list_history`/`add_history`/`remove_history` 到 `invoke_handler`
  - 加 `.setup(|app| { ... })`:取 `app_data_dir` → `create_dir_all` → `Connection::open` → `init_db` → 从 store 读旧 `AppConfig` 调 `migrate_from_store` → `app.manage(DbState(...))`
  - 保留 `tauri-plugin-store` 插件(迁移源)

**验证:**
```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

## 阶段 B — 后端单元测试(核心)

- [ ] B1. `db.rs` 内 `#[cfg(test)] mod tests`,用 `Connection::open_in_memory()`:
  - schema 建表 + 重复 `init_db` 幂等
  - config:`set`→`get` 等价;缺失 key 回退 default;部分更新
  - history:`add`→`list` 顺序(`created_at DESC`);`remove`;重复 id(`INSERT OR REPLACE`)
  - 迁移:`migrate_from_store` 内容一致 + meta 置位;**重复调用幂等**;空旧数据不报错;特殊字符(引号/emoji/长文本/空串)
- [ ] B2. 迁移函数签名设计为接受 `&Connection + Option<AppConfig>`,确保无需 mock Tauri 即可测

**验证:**
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## 阶段 C — 前端契约与页面

- [ ] C1. `src/lib/bindings.ts`:`AppConfig` 去掉 `history`;`emptyConfig` 同步去 history;新增 `listHistory(limit?, offset?, keyword?)` / `addHistory(item)` / `removeHistory(id)`
- [ ] C2. `src/lib/store.ts`:新增 `history` writable store;`initConfig` 内并行 `listHistory` 填充
- [ ] C3. `src/routes/history/+page.svelte`:列表读 `history` store(计数/空态随之改);删除调 `removeHistory(id)` 后更新 store
- [ ] C4. `src/routes/settings/+page.svelte`:`save` 的 `merged` 对象不再含 history(类型已去),其余逻辑不变
- [ ] C5. 主页生成成功后:前端同步把新条目推入 `history` store(或重新 `listHistory`),保证历史页即时可见

**验证:**
```bash
pnpm check
```

## 阶段 D — 前端纯逻辑测试

- [ ] D1. 加 `vitest` dev 依赖
- [ ] D2. 测 `src/lib/markdown.ts` `renderMarkdown`:基本渲染、有序列表序号、XSS sanitize
- [ ] D3. 测 `src/lib/template.ts` 常量非空

**验证:**
```bash
pnpm vitest run
```

## 阶段 E — 手动功能回归(必做)

启动 `pnpm tauri dev`,按「回归清单」逐项验证。

## 验证命令汇总

```bash
cargo test    --manifest-path src-tauri/Cargo.toml   # 后端单测(必过)
cargo build   --manifest-path src-tauri/Cargo.toml   # 后端编译
pnpm check                                            # 前端类型检查
pnpm vitest run                                       # 前端纯逻辑测试
pnpm tauri dev                                        # 手动回归
```

> Rust 工具链为自定义 `CARGO_HOME`/`RUSTUP_HOME`(`D:\Code\bin\rust`)+ rsproxy 镜像 + VS2022 MSVC(见 memory `rust-toolchain-setup`),`rusqlite` `bundled` 在该环境下可正常编译。

## 风险点与回滚

| 风险 | 对策 |
|---|---|
| **迁移损坏用户历史**(最高) | 迁移事务包裹 + `INSERT OR IGNORE` 幂等 + **不删 `data.json`** + 单测覆盖 + 用真实数据手测 |
| 重复启动重复迁移 | `meta.migrated_from_store` 标记,单测验证幂等 |
| `AppConfig` 去 history 的前端连锁 | 阶段 C 集中改 4 个消费点,`pnpm check` 兜底类型 |
| Windows rusqlite 编译 | `bundled` 自带 SQLite,VS2022 MSVC 已具备 |
| 配置项历史遗漏(KV key 映射不全) | config CRUD 单测逐 key 覆盖 |

**回滚**:迁移不删 `data.json`;回退代码版本后旧数据仍在;db 文件独立可删重建。

## 手动功能回归清单

- [ ] **全新环境**(删除/移走 db 与 data.json):首次启动空库正常,设置/历史/主页无报错,历史为空
- [ ] **旧数据迁移**:保留含历史/配置的 `data.json`、删除 db → 启动 → 历史条数与内容与迁移前一致、配置各字段一致
- [ ] **重复启动**:迁移后再次启动,历史不重复、数据稳定
- [ ] **设置页**:API 配置(baseUrl/key/model)保存→重读一致;「测试连接」成功;路径过滤(黑白名单)保存一致
- [ ] **采集**:选日期 → 采集对话(Claude Code)返回会话/token 估算;路径过滤生效
- [ ] **生成**:填要点(或采集)→ 流式生成日报,逐字呈现;完成后**历史自动新增**一条
- [ ] **历史页**:列表顺序正确(新→旧);「复用」回填主页输入框;「复制」入剪贴板;「查看」展开输入/日报;「删除」仅删该条、其余不变
- [ ] **导出**:配置了 exportDir → 直接写入并提示路径;未配置 → 弹窗选路径写入
- [ ] **类型/构建**:`cargo test`、`pnpm check`、`pnpm vitest run`、`pnpm tauri build` 均通过

## task.py start 前的检查

- [ ] `prd.md` / `design.md` / `implement.md` 已就绪且用户已审阅
- [ ] 上述四项验证命令(或等价)已在本机确认可行
- [ ] 迁移逻辑单测全部通过

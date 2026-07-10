# 引入 SQLite 重构数据持久层

## Goal

将当前基于 `tauri-plugin-store`(`data.json` 单文件、配置与历史混存、全量读写)的数据持久层,重构为 SQLite。重构后**所有现有功能必须保持等价、无回归**,并引入单元测试覆盖核心数据逻辑,为后续的历史搜索 / 分页 / 跨设备导入导出打基础。

## User Value

- 历史记录增长后不再拖慢启动与保存(当前每次 `load_config`/`save_config` 全量读写整个 JSON)。
- 配置与历史解耦,互不干扰。
- 为历史检索、分页、统计、导入导出提供可扩展的数据层。

## Confirmed Facts(代码探索)

- 技术栈:Tauri 2 + SvelteKit(Svelte 5 runes)+ Tailwind 4,Rust 后端。
- 当前存储:`tauri-plugin-store` → `data.json`,单一 `AppConfig` 结构体含 `history: Vec<HistoryItem>`。
- 读写模式:`load_config` 全量读、`save_config` 全量重写,无局部更新。
- 历史写入点:`src-tauri/src/llm.rs:175-185`(生成日报后 `load → insert(0) → save`)。
- 历史消费点:`src/routes/history/+page.svelte`(列/删)、`src/lib/store.ts`(全局 store)。
- 配置消费点:`src/routes/settings/+page.svelte`、`src/lib/store.ts`。
- 数据模型:`HistoryItem { id(UUID v4), date, title, input, output, created_at }`。
- **当前无任何自动化测试**:Cargo.toml 无测试依赖,package.json 无测试框架。

## Requirements(初步,待 brainstorm 收敛)

1. 引入 SQLite(rusqlite + bundled),建立数据层。
2. 历史记录迁移至 SQLite;配置存储的去向待定(见 Open Questions)。
3. 从旧 `data.json` 平滑迁移历史数据,迁移幂等、可验证、失败可回退(不删原文件)。
4. 现有 Tauri 命令的对外契约保持兼容(前端无感知地切换到新数据层);必要时拆分命令但不破坏功能。
5. 为数据层(迁移、CRUD、导入导出合并等纯逻辑)编写单元测试,覆盖大部分功能。
6. 表设计为后续导入导出预留(meta 版本表、稳定主键、KV 配置)。

## Acceptance Criteria(核心)

- [ ] 重构后**所有现有功能**逐项验证通过:配置读写、API 测试连接、采集对话、流式生成日报、历史列表/复用/复制/删除、导出 .md、路径过滤。
- [ ] 含旧 `data.json` 数据的环境,首次启动后历史被正确迁移到 SQLite,内容无丢失;重复启动不重复迁移。
- [ ] 全新环境(无 data.json)从空库正常工作。
- [ ] 数据层关键逻辑有单元测试覆盖并通过(CRUD、迁移、合并等)。
- [ ] `cargo test` 与前端类型检查 / 测试通过;构建与运行正常。

## Out of Scope

- 跨设备导入导出(本轮仅预留 schema:meta 版本表 + KV 配置 + 稳定主键)。
- 历史搜索 / 分页 / 按月统计 UI(本轮仅预留索引与查询能力)。
- 实时多端同步。
- 采集会话原文持久化(目前为临时态,不入库)。

## Decisions(已确认)

1. **范围边界**:本轮仅做 SQLite 数据层 + 旧 `data.json` 迁移 + 现有功能等价回归 + 数据层单元测试。导入导出、历史搜索/分页 UI 仅预留 schema 能力,留作后续独立任务。
2. **配置去向**:配置一并迁入 SQLite(KV 表)。`load_config`/`save_config` 对外签名不变,实现从 tauri-plugin-store 切到 SQLite;历史与配置共用同一数据源,可统一纳入单元测试。
3. **测试范围**:以 Rust 数据层(配置/历史 CRUD + 迁移)单元测试为核心;前端补纯逻辑函数测试(markdown、template);配一份逐项的手动功能回归验收清单覆盖涉及网络/LLM/文件的用户功能。不引入前端组件测试框架。
4. **`updated_at` 字段**:不加。当前无编辑历史功能(仅复用/复制/查看/删除),`updated_at` 会恒等于 `created_at`;未来需要时通过 `meta.schema_version` 迁移机制(ALTER TABLE ADD COLUMN)平滑添加。

## Open Questions

(全部已解决,见 Decisions)

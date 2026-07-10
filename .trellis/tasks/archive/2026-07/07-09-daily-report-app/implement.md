# 实现清单 - 日报生成 App

## 验证命令
- `pnpm install`
- `pnpm run tauri dev`（开发，启动桌面窗口）
- `pnpm run build`
- `cargo check --manifest-path src-tauri/Cargo.toml`

## 前置
- 项目当前非 git 仓库；建议先 `git init` 以便分阶段提交回滚（可选）。

## 执行步骤（有序）

### Phase A — 脚手架
- [ ] A1 用官方模板初始化 Tauri2 + Svelte + TS（`pnpm create tauri-app@latest`，选 Svelte / TypeScript）。
- [ ] A2 接入 TailwindCSS v4（`@tailwindcss/vite`）到 `app.css`；验证样式生效。
- [ ] A3 添加 Rust 依赖与 Tauri 插件：`reqwest`(rustls, stream), `tokio`, `serde`, `serde_json`, `chrono`, `uuid`, `tauri-plugin-store`, `tauri-plugin-dialog`, `tauri-plugin-fs`, `tauri-plugin-clipboard-manager`。
- [ ] A4 配置 `capabilities/default.json`（store / dialog / fs / clipboard 权限）。
- [ ] A5 `pnpm run tauri dev` 能起空白窗口。 ← REVIEW GATE 1

### Phase B — Rust 后端
- [ ] B1 `config.rs`：`AppConfig` / `ApiConfig` / `HistoryItem` 结构 + Store 读写（`load_config` / `save_config`）。
- [ ] B2 `commands.rs`：注册 `load_config` / `save_config` / `pick_directory` / `test_connection`。
- [ ] B3 `llm.rs`：`render_template` + OpenAI 兼容流式请求；`StreamChunk` + `Channel`。
- [ ] B4 `commands.rs`：`generate_report(input, Channel)` + 历史落库。
- [ ] B5 `export.rs`：`pick_export_path` / `write_text_file`；date → `yyyy-mm-dd`；`export_report`。
- [ ] B6 `cargo check` 通过。 ← REVIEW GATE 2

### Phase C — 前端 UI
- [ ] C1 `lib/bindings.ts`（invoke 封装 + TS 类型）、`lib/store.ts`（config / history / preview stores）、`lib/template.ts`（默认模板）、`lib/markdown.ts`。
- [ ] C2 `App.svelte`：顶部导航 主页 / 设置 / 历史；视图切换。
- [ ] C3 `MainView`：左输入框、右 marked 预览、生成 / 重生成 / 清空 / 复制 / 导出 按钮；Channel 流式接收。
- [ ] C4 `SettingsView`：API 配置 + 测试连接 + 模板编辑（恢复默认）+ 导出目录选择。
- [ ] C5 `HistoryView`：列表 + 复用（回填输入）/ 查看 / 删除。
- [ ] C6 错误 / 空状态提示、loading 状态。 ← REVIEW GATE 3

### Phase D — 集成验证
- [ ] D1 端到端：配置真实 API → 生成 → 流式预览 → 复制 / 导出 / 历史，全流程跑通。
- [ ] D2 错误路径：错误 Key / 断网 / 未配置 API。
- [ ] D3 跨平台冒烟（至少 Windows）。

## Review Gates
- Gate1：脚手架能起窗口。
- Gate2：Rust 编译通过，命令注册无误。
- Gate3：UI 三视图完成，可联调。

## Rollback Points
- 每个 Phase 结束可 `git commit`（若已 git init）。
- 后端 / 前端分层，前端 UI 出问题可单独回退而不动 Rust。

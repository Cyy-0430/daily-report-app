# 技术设计 - 日报生成 App

## 1. 架构总览

Tauri 2 应用：前端 Svelte 5 (WebView) + Rust 后端 (Tauri commands)。LLM 调用、配置存储、文件导出全部在 Rust 后端完成，前端只做 UI 与事件接收。

```
┌──────────────────────────────────────────────┐
│ Tauri 2 App                                   │
│  前端 (Svelte 5 + TS, WebView)                │
│   - 主页 / 设置 / 历史 三视图                  │
│   - invoke 调 Rust command                     │
│   - Channel 接收流式分片                       │
│  ──────────────────────────────────────────── │
│  Rust 后端                                     │
│   commands.rs  - Tauri command 入口 / 注册     │
│   llm.rs       - OpenAI 兼容流式调用           │
│   config.rs    - 配置/历史持久化 (Store 插件)  │
│   export.rs    - 导出 .md、选目录、写文件      │
└──────────────────────────────────────────────┘
```

## 2. 技术选型

| 项 | 选择 | 理由 |
|---|---|---|
| 桌面框架 | Tauri 2 | 跨平台、体积小、Rust 后端，移动端可复用 |
| 前端 | Svelte 5 + TypeScript | 用户指定；轻量、响应式 |
| 构建 | Vite | Tauri 官方推荐 |
| 样式 | TailwindCSS v4 (`@tailwindcss/vite`) | 配置简单、CSS-first |
| Markdown 渲染 | `marked` + `DOMPurify` | 轻量；预览防 XSS |
| HTTP (Rust) | `reqwest` (rustls, stream) | 流式、跨平台、避免 OpenSSL 依赖 |
| 异步运行时 | `tokio` | reqwest 流式所需 |
| 存储 | `tauri-plugin-store` | 简单 KV JSON，足够 |
| 对话框/文件 | `tauri-plugin-dialog` / `tauri-plugin-fs` | 选目录、写文件 |
| 剪贴板 | `tauri-plugin-clipboard-manager` | 复制 |
| 流式通道 | `tauri::ipc::Channel` | Rust → 前端推流分片 |

## 3. 数据模型 & 存储

存储位置：app data dir 下的 `data.json`（通过 Store 插件）。结构：

```json
{
  "apiConfig": {
    "baseURL": "https://api.openai.com/v1",
    "apiKey": "sk-...",
    "model": "gpt-4o-mini"
  },
  "promptTemplate": "... 含 {{input}} {{date}} ...",
  "exportDir": "C:/Users/.../reports",
  "history": [
    {
      "id": "uuid",
      "date": "2026-07-09",
      "title": "7.9日报",
      "input": "...",
      "output": "...",
      "createdAt": 0
    }
  ]
}
```

## 4. 契约：Tauri Commands

前端 `invoke` 的命令签名（Rust）：

- `load_config() -> AppConfig` — 读取全部配置（Key 也在内，仅设置页用）。
- `save_config(config: AppConfig) -> Result<()>` — 保存配置。
- `test_connection(api: ApiConfig) -> Result<String>` — 测试连通，返回成功消息或 error。
- `generate_report(input: String, on_event: Channel<StreamChunk>) -> Result<()>`
  - 在 Rust 内读取已保存的 `apiConfig` + `promptTemplate`（Key 不经前端）。
  - 渲染模板（替换 `{{input}}` `{{date}}`）→ 组成 `messages`。
  - `POST {baseURL}/chat/completions` with `stream:true`。
  - 逐 chunk 通过 Channel 推 `Delta(String)`；结束推 `Done`；出错推 `Error(String)`。
- `pick_directory() -> Option<String>` — 设置页选导出目录（弹 dialog）。
- `export_report(content: String, date: String) -> Result<String>`
  - 若 `exportDir` 非空：写入 `{exportDir}/{date}.md`（date 为 `yyyy-mm-dd`）。
  - 若 `exportDir` 为空：前端先调 `pick_export_path` 拿路径，再调 `write_text_file`。
  - 返回最终写入路径。

辅助：
- `pick_export_path(default_name: String) -> Option<String>` — 弹保存对话框。
- `write_text_file(path: String, content: String) -> Result<()>`

`StreamChunk`（serde tagged enum）：
```rust
#[derive(Serialize)]
#[serde(tag = "type")]
enum StreamChunk {
    Delta { text: String },
    Done,
    Error { message: String },
}
```

## 5. 关键数据流：流式生成

1. 前端：用户点「生成」→ `invoke('generate_report', { input, onEvent: channel })`，传入新建 `Channel`。
2. Rust：拼 `messages`（system + user=渲染后模板）→ reqwest POST → `bytes_stream()`。
3. Rust：解析 SSE 行 `data: {...}`，取 `choices[0].delta.content`，`channel.send(Delta)`。
4. 前端：`onmessage` 追加 delta 到预览 store，`marked` 增量重渲染。
5. 结束：Rust `send(Done)`；前端把完整 output 通过 `add_history` 存入历史。

## 6. 模板变量渲染

- `{{input}}` ← 左侧输入框内容。
- `{{date}}` ← 今天，正文格式 `月.日`（如 `7.9`）。
- 文件名日期：单独 `yyyy-mm-dd`（如 `2026-07-09`），Rust 端 `chrono::Local::now()` 生成。
- 渲染在 Rust 端进行（`generate_report` 内），保证一致性。

## 7. 边界与错误

- baseURL 规范化：去尾部斜杠；若已以 `/v1` 结尾则拼 `/chat/completions`，否则也补全路径。
- reqwest 超时（连接 + 整体）；流式读取循环遇 `data: [DONE]` 结束、遇 error 报错。
- 未配置 API：前端在生成前拦截，提示去设置页。
- 导出文件已存在：MVP 直接覆盖并提示（后续可加确认）。
- `baseURL` / `model` 缺失：`test_connection` / `generate_report` 返回明确错误。

## 8. 移动端兼容性预留

- 所有 LLM / 存储 / 导出逻辑在 Rust，移动端可直接复用。
- 前端不依赖浏览器 `fetch` 调 LLM，便于移动端复用。
- `dialog` / `fs` 在移动端行为不同，后续阶段适配。

## 9. 安全

- API Key 仅存本地 Store；前端除设置表单外不读取 Key（`generate_report` 在 Rust 内读）。
- Markdown 渲染用 DOMPurify 防 XSS（虽是本地内容，仍保留）。

## 10. 项目结构（最终实现）

前端使用 SvelteKit（adapter-static，SPA 模式，`ssr=false`），非纯 Svelte SPA：

```
daily-report-app/
├── package.json, vite.config.js, svelte.config.js, tsconfig.json
├── index.html
├── src/
│   ├── app.css                 # Tailwind v4 入口 + 设计系统（纸本档案风）
│   ├── app.html
│   ├── lib/
│   │   ├── bindings.ts         # invoke 封装 + TS 类型 + Channel 流式
│   │   ├── store.ts            # svelte stores: config / toast / pendingInput
│   │   ├── template.ts         # 默认提示词模板
│   │   └── markdown.ts         # marked + DOMPurify 渲染
│   └── routes/                 # SvelteKit 路由
│       ├── +layout.svelte      # 顶栏(自定义标题栏) + 导航 + toast
│       ├── +page.svelte        # 主页：左输入 / 右预览(可编辑)
│       ├── settings/+page.svelte
│       └── history/+page.svelte
└── src-tauri/
    ├── Cargo.toml, tauri.conf.json, build.rs
    ├── capabilities/default.json
    └── src/
        ├── main.rs, lib.rs     # lib.rs 注册 plugin + invoke_handler
        ├── config.rs           # load_config / save_config
        ├── llm.rs              # test_connection / generate_report（流式）
        └── export.rs           # export_report / write_text_file
```

## 11. 实现备注（相对初版设计的调整）

- **前端框架**：实际采用官方模板的 SvelteKit + adapter-static（SPA 模式），而非纯 Svelte；用 SvelteKit 路由组织 主页/设置/历史。
- **Command 分布**：未单独建 `commands.rs`，Tauri command 内聚在各业务模块（`config`/`llm`/`export`），由 `lib.rs` 统一注册。
- **dialog / clipboard 由前端 JS 调用**：选导出目录（`plugin-dialog` 的 `open`）、保存弹窗（`save`）、复制（`plugin-clipboard-manager` 的 `writeText`）都在前端完成；Rust 只负责配置存储、LLM 流式调用、`std::fs` 写文件。相应 capability 已授权 `dialog:allow-open/allow-save`、`clipboard-manager:allow-write-text`。
- **无边框窗口**：`decorations:false` + 自定义标题栏（`data-tauri-drag-region` 可拖拽，右侧最小化/最大化/关闭按钮），顶部完全融入米色主题。
- **设计系统**：纸本档案风——暖米纸底 `#f3eee3`、墨色字、赭红 `#9c3a26` 唯一强调色；等宽字用于标签/编号/计数；两框 head/body/foot 三段镜像严格对齐。
- **日期格式**：正文 `{{date}}` → `月.日`（如 `7.9`）；导出文件名 → `yyyy-mm-dd.md`。

# 日报生成 · Daily Report

一个基于 **Tauri 2** 的跨平台桌面应用：左侧写下「今天做了什么」，右侧通过你自配的 OpenAI 兼容 API **流式**生成符合自定义模板格式的日报 Markdown，支持编辑、复制、导出与历史记录。

## ✨ 功能

- **左输入 / 右流式预览**：打字机式逐字呈现生成过程
- **可编辑日报**：生成后可切换「编辑」直接改 Markdown 源码，复制 / 导出用改后内容
- **导出 `.md`**：文件名默认 `yyyy-mm-dd.md`，可配置固定导出目录或每次弹窗选择
- **自定义 API**：OpenAI 兼容格式（OpenAI / DeepSeek / 通义 / Moonshot / 本地 Ollama 等皆可），附「测试连接」
- **自定义提示词模板**：支持 `{{input}}`、`{{date}}` 变量，一键恢复默认
- **历史记录**：生成后自动保存，支持复用（回填输入）、查看、删除
- **无边框自定义标题栏** + 纸本档案风 UI（暖米纸 / 墨色 / 赭红）

## 🧱 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | SvelteKit + Svelte 5 + TypeScript（adapter-static，SPA 模式） |
| 样式 | TailwindCSS v4 |
| Markdown | marked + DOMPurify |
| 后端 | Rust：reqwest(rustls, stream) 流式调用 + Tauri Channel 转发；tauri-plugin-store 本地存储 |

## 🚀 开发

前置：Node.js、pnpm、Rust（MSVC 工具链）。

```bash
pnpm install
pnpm tauri dev      # 启动开发（编译 Rust + 打开窗口）
pnpm tauri build    # 打包发布
```

## 🔒 安全

API Key 仅存本地，LLM 调用全部走 Rust 后端，Key 不进入前端 JS 运行时（配置表单除外）。

## 📐 设计

纸本档案（Editorial Paper）风格：暖米纸底、墨色正文、赭红（terracotta）为唯一强调色，等宽字用于标签 / 编号 / 计数，营造手账档案感。两个主面板采用 head/body/foot 三段镜像结构以保证严格对齐。

## License

MIT

# 日报生成 · Daily Report

一个基于 **Tauri 2** 的跨平台桌面应用:**自动采集本地 AI 编程工具的当天对话**,结合你自配的 OpenAI 兼容 API 一键流式生成格式化的工作日报 / 周报。今天写了什么代码、调了什么问题,不用回忆、不用手写——它自己知道。

## ✨ 核心亮点:自动采集,零手写要点

- **读本地对话日志**:自动解析 Claude Code、ZCode、Codex、OpenCode 四款 AI 编程工具在本机的会话记录,提取你当天的提问、改动与工具调用
- **零手动、可组合**:一键采集即可成报;也支持手写要点,两者可与提示词模板自由组合
- **纯本地、零额外 token**:采集只读本机文件、不产生任何网络请求,不花一分钱 token;只有最终生成报告时才调用你配置的 LLM
- **按项目过滤**:基于会话真实工作目录(cwd)设置白名单 / 黑名单,排除优先,子目录自动纳入——私人项目绝不混进日报
- **token 实时估算**:采集结果即时刻估算 token 量,发送前心里有数

## 🧰 功能一览

- **日报 / 周报双模式**:周报自动汇总整周的历史日报,map(逐日摘要)+ reduce(整周汇总)双阶段提示词,各自可自定义
- **左输入 / 右流式预览**:打字机式逐字呈现生成过程;生成后可切「编辑」直接改 Markdown,复制 / 导出用改后内容
- **导出 `.md`**:文件名默认 `yyyy-mm-dd.md`,可配置固定导出目录或每次弹窗选择
- **自定义 API**:OpenAI 兼容格式(OpenAI / DeepSeek / 通义 / Moonshot / 本地 Ollama 等皆可),附「测试连接」,支持网络代理(同代理检查更新)
- **自定义提示词模板**:支持 `{{input}}`(要点)、`{{conversations}}`(采集对话)、`{{date}}` 变量
- **历史记录**:生成后自动保存(本地 SQLite),支持复用回填、查看、删除
- **主题定制**:内置「纸墨」主题,10 项颜色变量(背景 / 卡片 / 正文 / 强调色等)可视化调色盘定制,支持预览、多主题保存与随时切换,可导出/导入 JSON 分享主题
- **桌面体验**:无边框自定义标题栏、纸感 UI、自动检查更新、单实例、窗口状态记忆

## 🧱 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | SvelteKit + Svelte 5 (runes) + TypeScript（adapter-static，SPA 模式） |
| 样式 | TailwindCSS v4 + CSS 变量主题系统 |
| 调色盘 | svelte-awesome-color-picker |
| Markdown | marked + DOMPurify |
| 后端 | Rust：reqwest(rustls) 流式调用 + Tauri Channel 转发；rusqlite(SQLite) 本地存储 |
| 对话采集 | Rust 原生解析各工具本地会话日志(JSONL / SQLite),按行级时间戳过滤到目标日 |

## 🚀 开发

前置：Node.js ≥ 24、pnpm、Rust（MSVC 工具链）。

```bash
pnpm install
pnpm tauri dev      # 启动开发（编译 Rust + 打开窗口）
pnpm tauri build    # 打包发布

pnpm check          # 前端类型检查(svelte-check)
pnpm test           # 前端单元测试(vitest)
cargo test          # Rust 测试(在 src-tauri/ 下)
```

## 🔒 安全

- API Key 仅存本地,LLM 调用全部走 Rust 后端,Key 不进入前端 JS 运行时(配置表单除外)
- 对话日志仅在本机读取与渲染,采集流程无任何网络请求

## 📐 设计

纸本档案(Editorial Paper)风格:暖米纸底、墨色正文、赭红(terracotta)为唯一强调色,等宽字用于标签 / 编号 / 计数,营造手账档案感。两个主面板采用 head/body/foot 三段镜像结构以保证严格对齐。内置预设主题「纸墨」即该风格,并可在设置中整体换色。

## License

MIT

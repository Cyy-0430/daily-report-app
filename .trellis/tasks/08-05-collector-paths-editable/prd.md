# 采集工具路径可编辑与恢复默认

## Goal

在设置页「采集工具」区，为每个采集工具（claude-code / codex / zcode / opencode）新增**可编辑的数据源路径**，并在每行右侧提供独立的「恢复默认」按钮，让用户在工具被装到非默认位置（或想指向其他实例）时能自行指定数据源，无需改代码。

## Background

当前四个采集器的数据源路径全部硬编码在后端：
- claude-code → `~/.claude/projects`（目录）
- codex → `~/.codex/sessions`（目录）
- zcode → `~/.zcode/cli/db/db.sqlite`（文件）
- opencode → `~/.local/share/opencode/opencode.db`（文件，XDG 回退）

用户若把工具装到别处（如 `D:\Tools\.claude`）或多实例共存，目前无法配置，采集会静默取空或报错。

## Requirements

### 功能需求
- **R1 可编辑**：设置页每个采集工具一行，展示其当前生效的数据源路径，可直接在输入框中编辑文本。
- **R2 恢复默认**：每行右侧一个「恢复默认」按钮，点击后该行路径恢复为该工具的默认值。
- **R3 真实默认值**：默认路径必须由后端解析后返回（展开 `~` 为真实主目录），不能在前端硬编码——opencode 还有主路径/XDG 回退逻辑，只有后端能给出权威默认。
- **R4 持久化**：编辑后的路径随配置保存到 SQLite（`config` 表），下次启动恢复。空/等于默认 = 用默认（不存覆盖）。
- **R5 采集生效**：日报生成时，采集流程使用各工具的覆盖路径（若有），否则用默认。
- **R6 `~` 展开**：输入框支持 `~` / `~/...` 写法（后端展开为主目录），也支持绝对路径。
- **R7 向后兼容**：老配置无此字段时按默认行为运行，`load_config` 能正常 round-trip（`#[serde(default)]`）。

### 非目标（Out of Scope）
- 不做文件夹/文件选择器按钮（两个工具是目录、两个是文件，选择器语义不一致；仅做文本编辑 + 恢复默认）。
- 不改变各采集器「路径不存在」时的既有错误语义（claude-code 仍报错上抛；zcode/codex/opencode 仍静默跳过）。
- 不引入环境变量 / 多实例列表 / 自动发现机制。

## Acceptance Criteria

- [ ] **AC1**：设置页「采集工具」每个工具下出现一行可编辑路径输入框，初始显示该工具的真实默认路径（如 `C:\Users\<u>\.claude\projects`），右侧有「恢复默认」按钮。
- [ ] **AC2**：编辑某工具路径为自定义值 → 保存 → 重启应用 → 该工具仍显示自定义值。
- [ ] **AC3**：点击「恢复默认」→ 该行回到默认路径；保存后配置中该工具不再存覆盖（存 `""` 或不存）。
- [ ] **AC4**：把 claude-code 指向一个含 jsonl 的自定义目录 → 在生成页采集，能取到该目录下的会话；opencode/zcode 指向自定义 db 文件同理。
- [ ] **AC5**：输入 `~/some/path` 形式的路径，后端正确展开为主目录下的子路径。
- [ ] **AC6**：老配置（无 `toolPaths` 字段）加载不报错，所有工具走默认。
- [ ] **AC7**：`pnpm check`（svelte-check）与 `cargo test` 全绿；新增的 `expand_home` 单测通过。
- [ ] **AC8**：`collector-spec.md` 已同步新签名（`collect` 加 `custom_path`、`default_path` trait 方法、`default_collect_paths` 命令、`toolPaths` 配置字段、`~` 展开契约）。

## Notes

- 跨层契约变更：`CollectConfig` 新增 `toolPaths`；`collect_conversations` 新增 `toolPaths` 入参；`Collector::collect` 签名变更；新增 `default_collect_paths` 命令。Rust ↔ TS 必须手工同步（无 codegen）。
- 触及采集器核心契约，须遵守 `.trellis/spec/backend/collector-spec.md`（真实 cwd 过滤、按行 timestamp 过滤等不变量保持不变——本任务只改「数据源定位」，不动过滤/解析）。

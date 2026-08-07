# 提取重复/魔法字符串为常量

## Goal

把散落在前后端代码里的**重复字面量**与**特殊语义字符串**提取为命名常量，消除"改一处漏一处"
导致的契约漂移与静默 bug（典型：db.rs 配置键镜像）。**纯重构，运行时行为零变更。**

## Background

上一轮已通读全部源码（Rust 11 + TS/Svelte 10），归纳出待提取清单并经用户确认"都改"。
最危险的重复是 db.rs 中 `get_config`(L86–112) 与 `config_pairs`(L128–137) 对 9 个配置键
的镜像抄写——漏改任一处即读写不对称、字段静默丢失。

## Scope

**纳入：**
- **A 组（强烈建议，高频/跨层契约）：**
  - A1 配置 KV 键（db.rs 两处镜像）
  - A2 meta 键 + 哨兵值（schema_version / migrated_from_store / "1"）
  - A3 工具 id 四件套（后端 11+ 处、前端 3 处，跨层契约）
  - A4 chrono 日期格式串（%Y-%m-%d / %H:%M / %Y-%m-%d %H:%M，10+ 处）
  - A5 LLM 错误/提示消息（多处重复：API 未配置、请求失败、无法定位主目录等）
  - A6 模板变量占位符（{{date}} 等，render 函数内重复）
  - A7 LLM 端点片段 + SSE 标记（/chat/completions / data: / [DONE]）
- **B 组（建议，特殊语义/中频）：** DB/store 文件名、进度 stage（map/reduce，跨层）、
  前端"请先在设置中配置 API"重复文案、周报分隔符（~/、/（无））、title 后缀（日报/周报）。
- **顺带（非字符串魔法值）：** 截断长度魔法数 `80`（6 处）提为 const；完全相同的
  `truncate` 函数（3 份）合并到 collector/mod.rs 共用。

**排除（附理由，即使"都改"也不动）：**
- **TS discriminated union 的 `type` 字面量**（`delta/done/error/progress`）：嵌入类型定义，
  抽成 `const` 会破坏字面量收窄与 switch 穷尽性检查，**有损类型安全**。
- **测试 fixture 中的纯数据字符串**（如 db.rs `sample_config` 的 `"gpt-demo"`/`"sk-xxx"`/
  `"https://api.example.com/v1"`）：属测试输入数据，非魔法值，提常量反而降低测试可读性。
  （注：工具 id 在测试里的重复**会**改——引用生产常量，保持单一来源。）

**C 组其余（低收益，默认纳入但放最后）：** settings 展示用的模板变量文本 `{{date}}` 等、
单次 UI 文案——提取收益有限，按"都改"纳入，置于实现末尾。

## Requirements

- R1 每一处纳入项的字面量替换为命名常量，常量就近定义在使用模块（Rust）/派生自现有
  单一来源（前端工具 id 派生自 `COLLECT_TOOLS`）。
- R2 跨层契约字符串（工具 id、进度 stage）前后端定义对齐，且后端工具 id 单一来源。
- R3 **行为零变更**：不修改任何控制流、不改变任何字符串的运行时值；既有测试不动且全绿。
- R4 顺带项：`truncate` 三合一到 `collector/mod.rs`（各 collector 改为引用），魔法数 `80` 提常量。

## Acceptance Criteria

- [ ] AC1 `cargo test`（src-tauri/）既有测试**全绿、零改动**——纯重构零回归的硬证据。
- [ ] AC2 `cargo check` + `pnpm check` 通过，无新增警告。
- [ ] AC3 grep 复核：db.rs 不再出现两份配置键字面量；`["claude-code","zcode","codex","opencode"]`
      字面量数组在前端只剩 COLLECT_TOOLS 一处定义，其余派生。
- [ ] AC4 工具 id / stage 等跨层契约字面量在后端有唯一 const 定义，各引用点指向它。
- [ ] AC5 `truncate` 仅一份（在 collector/mod.rs）；魔法数 `80` 全部由常量替换。

## Notes

- 跨层同步契约见 CLAUDE.md "Cross-layer conventions"：Rust 结构 ↔ TS interface 手工同步，
  本任务不新增字段故无同步负担，但工具 id 常量化需确认两侧值一致。

# Implement — 采集路径过滤

> 关联 `prd.md` / `design.md`。按序执行,每步后跑对应校验。

## 校验命令

- 后端单测/编译:`cd src-tauri && cargo test && cargo build`
- 前端类型检查:`npm run check`
- 全链路:`npm run build`(可选,改 UI 后建议跑)
- 手测:`npm run tauri dev`,设置页配置路径后点「采集对话」验证效果

## 实现清单(按依赖序)

### 1. Rust 配置字段 `config.rs`
- [ ] `CollectConfig` 增 `include_paths` / `exclude_paths`(均 `#[serde(default)]`)。
- [ ] `Default for CollectConfig` 补两字段为 `vec![]`。

### 2. Rust 过滤核心 `collector/claude_code.rs`
- [ ] 加 `norm(p: &str) -> PathBuf`(分隔符统一 `\`、小写、去尾部分隔符)。
- [ ] 加 `session_allowed(cwd: Option<&Path>, includes: &[PathBuf], excludes: &[PathBuf]) -> bool`:
      includes 非空先过白名单,再过黑名单(排除优先);cwd=None 时按 design §3 规则。
- [ ] 单测(work/workplace 边界、子目录、空规则、`\` vs `/`、大小写、cwd=None)。
- [ ] `ClaudeCodeCollector::collect` 签名加 `filter: &PathFilter`;push 前判 `session_allowed`。

### 3. Rust 路由 `collector/mod.rs`
- [ ] 加 `pub struct PathFilter { includes, excludes }`(及从字符串构造的 helper,内部调 `norm`)。
- [ ] `Collector::collect` 签名加 `filter: &PathFilter`。
- [ ] `collect_blocking(date, tools, filter)` 透传 filter。
- [ ] `collect_conversations` 命令增 `filter` 参数(`#[tauri::command]`),透传。

### 4. TS 绑定 `src/lib/bindings.ts`
- [ ] `CollectConfig` 增 `includePaths` / `excludePaths`。
- [ ] 加 `PathFilter` 类型;`collectConversations(date, tools, filter)` 改签名。
- [ ] `emptyConfig()` 补两字段默认空数组。

### 5. 前端调用点 `src/routes/+page.svelte`
- [ ] `onCollect` 从 `$config.collectConfig` 读 `includePaths`/`excludePaths`,
      组 `filter` 传入 `collectConversations`(默认空数组,兼容旧 config)。

### 6. 设置页 UI `src/routes/settings/+page.svelte`
- [ ] D 区块下新增「路径过滤」子区:排除路径、仅采集路径两组可增删列表
      (每行 文本框 + 「选择…」目录选择 + 「✕」)。
- [ ] `onMount` 读取 `c.collectConfig.includePaths/excludePaths` 初始化本地状态。
- [ ] `save()` 的 `merged.collectConfig` 补 `includePaths` / `excludePaths`。
- [ ] 提示文案注明子目录包含、排除优先。

## Review Gate(实现后、`task.py start` 前已就位即跳过)

- [ ] `prd.md` 验收项逐条可测;`design.md` 契约与代码一致;本清单全勾。

## 风险点 / 回滚

- `Collector::collect` 签名变更 → 仅一个实现,改完即可编译验证。
- `collect_conversations` 命令签名变 → 前端唯一调用点 `+page.svelte` 同步改,否则 invoke 报错。
- 旧 data.json 兼容靠 `#[serde(default)]`;若手测发现旧配置加载异常,检查 Default impl 是否补齐空数组。
- 回滚:还原 `config.rs` / `mod.rs` / `claude_code.rs` / `bindings.ts` / 两页 svelte(6 文件)。

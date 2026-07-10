# Design — 采集路径过滤

> 关联 `prd.md`。本文件记录技术设计:边界、契约、数据流、兼容性、取舍。

## 1. 总体策略

过滤基于每个 session 的**真实 cwd**(未编码的工作目录,如 `D:\aaaa`),用**组件级
前缀匹配**做黑白名单。完全不碰编码后的目录名,从根上规避连字符/中文歧义。

过滤发生在 `ClaudeCodeCollector::collect()` 内,**parse_session 之后、push 之前**:
拿到 `SessionDigest.cwd` 即可判定,无需侵入解析逻辑(单一职责)。

## 2. 数据流

```
前端 settings: includePaths[] / excludePaths[]  ──save──▶  data.json (CollectConfig)
                                                              │
采集时:                                                        ▼
+page.svelte onCollect()
  ├ 读 $config.collectConfig.{includePaths, excludePaths}
  └ collectConversations(date, tools, filter)   filter = { includePaths, excludePaths }
        │  (invoke)
        ▼
collect_conversations(date, tools, filter)  [tauri command]
  └ spawn_blocking ▶ collect_blocking(date, tools, filter)
        ├ 构建 PathFilter(规范化:分隔符统一 + Windows 小写)
        ├ for each Collector: c.collect(date, &filter)
        │     └ ClaudeCodeCollector::collect:
        │         parse_session() → digest
        │         if digest.lines 非空 && session_allowed(digest.cwd, filter):
        │             digests.push(digest)     ◀ 过滤在此,单点
        └ render / 汇总(不变)
```

**关键契约不变**:`CollectResult` / `SessionDigest` / `ConversationLine` 结构与渲染逻辑
(`mod.rs::render`)均不动;过滤只影响「哪些 session 进入 digests」。

## 3. 核心纯函数(可单测,放在 `claude_code.rs`)

```rust
/// 规范化:统一分隔符为 '\\'(Windows),整体小写,去尾部分隔符。
fn norm(p: &str) -> PathBuf

/// 判定单个 session 是否被允许采集。
/// - includes 非空:cwd 必须落在任一 include 下(含自身),否则拒绝。
/// - 命中任一 exclude:拒绝(排除优先)。
/// - cwd 为 None:includes 非空→拒绝(无法证实白名单);否则放行。
fn session_allowed(cwd: Option<&Path>, includes: &[PathBuf], excludes: &[PathBuf]) -> bool
```

匹配语义:`norm(cwd) == norm(base)` 或 `norm(cwd).starts_with(norm(base) + 分隔符)`。
- 组件级前缀,子目录继承(`D:\work` 命中 `D:\work\sub`)。
- 用「`base` + 分隔符」做前缀,避免 `work` 误命中 `workplace`。
- 小写化解决 Windows 大小写;分隔符统一解决 `\` vs `/`。

> `Path::starts_with` 在 Windows 上**不自动**大小写折叠,故显式小写化两边。

## 4. 类型与契约改动

### Rust (`config.rs`)
```rust
pub struct CollectConfig {
    #[serde(default = "default_enabled_tools")]
    pub enabled_tools: Vec<String>,
    #[serde(default)]
    pub include_paths: Vec<String>,   // 仅采集(白名单),空=不限
    #[serde(default)]
    pub exclude_paths: Vec<String>,   // 排除(黑名单)
}
// Default impl 同步补 include_paths/exclude_paths = vec![]。
```

### Rust (`collector/mod.rs`)
```rust
/// 路径过滤规则(规范化后的路径)。
pub struct PathFilter { pub includes: Vec<PathBuf>, pub excludes: Vec<PathBuf> }

pub trait Collector: Send + Sync {
    fn collect(&self, date: NaiveDate, filter: &PathFilter)
        -> Result<(Vec<SessionDigest>, usize), String>;
}
// collect_blocking / collect_conversations 增加 filter 参数,透传给 collector。
```

### TS (`bindings.ts`)
```ts
export interface CollectConfig {
  enabledTools: string[];
  includePaths: string[];   // 仅采集
  excludePaths: string[];   // 排除
}
export interface PathFilter { includePaths: string[]; excludePaths: string[]; }
export const collectConversations = (date: string, tools: string[], filter: PathFilter) =>
  invoke<CollectResult>("collect_conversations", { date, tools, filter });
// emptyConfig() 补 includePaths/excludePaths = []
```

## 5. 兼容性 / 迁移

- `#[serde(default)]` + Default impl 补默认空数组 → **旧 data.json 无新字段时等价于不过滤**,完全向后兼容。
- `Collector::collect` 签名变更:当前仅 `ClaudeCodeCollector` 一个实现,改动面可控。
- `collect_conversations` 命令签名新增 `filter` 参数:前端调用点仅 `+page.svelte:56` 一处。

## 6. UI(设置页 D 区块下新增「路径过滤」子区)

两组可增删的路径列表(排除 / 仅采集),每行 = 文本输入 + 「选择…」(复用
`open({directory:true})`)+ 「✕」。保存逻辑在已有 `merged.collectConfig` 合并点
(`settings/+page.svelte:35`)补 `includePaths` / `excludePaths`。

提示文案:「子目录会被一并包含/排除;排除优先于仅采集。」

## 7. 取舍 / 回滚

- **不做目录名预过滤**:换来代码简洁与零歧义;代价是被排除项目仍被枚举/解析,
  但量级小,可接受。若日后项目数极多,可在 `collect()` 遍历目录时加一层「编码名预筛」
  作性能优化(独立、可回滚,不影响过滤正确性)。
- **过滤点选 digest 之后**而非 parse 内早返回:保持 parse_session 单一职责;解析很快。
- 回滚点:纯函数 + 配置字段均为增量;回滚只需还原 4 个文件。

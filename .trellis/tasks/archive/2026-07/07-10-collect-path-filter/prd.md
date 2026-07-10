# 采集路径过滤(排除/仅采集 黑/白名单)

## Goal

让用户能按「真实工作目录(cwd)」对 Claude Code 采集到的对话做路径过滤:支持
**排除(黑名单)**——某些目录下的会话一律不采集;以及**仅采集(白名单)**——
只采集指定目录下的会话。两种列表均可配置多条路径。

## User Value

- 避免把与工作无关 / 隐私项目(如 `D:\aaaa`、个人项目)的对话写进日报。
- 只聚焦工作目录(如 `D:\work`),减少噪声、压缩 token、保护隐私。

## Confirmed Facts (经代码勘察)

- 数据源:`~/.claude/projects/<编码路径>/*.jsonl`,入口 `claude_code.rs:27` `home_projects_dir()`。
- 目录名是**编码后**的项目路径:`:` `\` `/` → `-`(本机实测:`D:\Easy`→`D--Easy`)。
- 编码**有歧义**:中文/特殊字符变成连串 `-`,且与真实路径里的 `-` 无法区分
  (本机存在 `D--Easy-------`、`D--Easy---------` 等)。→ **不能用目录名做匹配**。
- 每个 session 的 jsonl 行内含**真实未编码的 `cwd`** 字段,代码已在读(`claude_code.rs:129-133`)。
- 当前采集仅按「日期 + 工具勾选」过滤(`mod.rs:152` `collect_blocking`),**无任何路径过滤**。
- 配置走 `CollectConfig`(Rust `config.rs:19` / TS `bindings.ts:9`),存于 tauri store `data.json`;
  已有 `#[serde(default)]` 兼容旧字段缺失的先例(`config.rs:23` `enabled_tools`)。
- 采集命令 `collect_conversations(date, tools)`,前端只传 date+tools(`bindings.ts:82`)。
- 设置页 D 区块已有「选择目录」对话框 `open({directory:true})`(`settings/+page.svelte:77`)可复用。

## Requirements

- [ ] 扩展 `CollectConfig` 新增 `include_paths: Vec<String>` 与 `exclude_paths: Vec<String>`
      (Rust + TS 双端;`#[serde(default)]` 兼容旧配置)。
- [ ] 采集时基于每个 session 的**真实 cwd**做过滤(组件级前缀匹配,子目录继承)。
      - include 非空:仅保留 cwd 落在任一 include 路径下(含自身)的会话。
      - exclude:剔除 cwd 落在任一 exclude 路径下的会话。
- [ ] 效率:命中排除时尽量早返回(读取首行 cwd 后即可跳过整个文件,避免全量解析)。
- [ ] 设置页新增「路径过滤」UI:排除路径、仅采集路径两组可增删的路径列表,复用目录选择对话框。
- [ ] 抽纯函数 `session_allowed(cwd, includes, excludes)` 并补单测(work/workplace 边界、
      子目录、空规则、Windows 大小写/分隔符)。

## Acceptance Criteria

- [ ] 在「排除」中加入 `D:\aaaa`,采集时该目录下会话不出现;`D:\aaaa\sub` 同样被排除。
- [ ] 「仅采集」设为 `D:\work`,只有 `D:\work` 及其子目录下的会话被采集。
- [ ] 两条路径以上可正常生效;空列表 = 不过滤(默认行为不变,向后兼容)。
- [ ] 旧配置(无新字段)加载与采集均正常,等价于不过滤。
- [ ] 路径分隔符混用(`D:\work` 与 `D:/work`)、大小写差异不影响匹配。
- [ ] `session_allowed` 纯函数单测全绿。

## Out of Scope

- 目录名编码预过滤(纯性能优化,MVP 不做;见 open question)。
- 路径存在性校验(纯字符串/组件语义匹配,不读盘)。
- 其它工具的路径过滤(MVP 仅 claude-code;架构上 Collector 可扩展)。
- 正则 / glob 高级匹配(仅前缀/目录包含)。

## Resolved Decisions

- **优先级:排除优先。** include 非空 → 先过白名单(cwd 必须落在某条 include 下);
  再过黑名单(命中任一 exclude 则丢弃)。即「在白名单范围内挖掉黑名单」,
  保证敏感目录绝不会进日报。
  - 示例:`include=[D:\work]`、`exclude=[D:\work\secret]` →
    `D:\work\app` 采集、`D:\work\secret` 排除、`D:\personal` 排除(不在白名单)。

## Resolved Decisions (续)

- **不做目录名预过滤,保持简洁。** 仅做一层过滤:解析得到 session 的真实 cwd 后,
  用 `session_allowed()` 判定是否保留。不为性能引入「编码目录名前缀匹配」那一层
  (它需复现 Claude 编码规则、且有连字符/中文歧义)。被排除的 session 仍会被解析,
  但项目数有限、解析很快,可接受。

## Open Questions

- (无。两处用户决策均已收敛:排除优先、不做预过滤。)

## 实现侧勘察事实(供 design 参考)

- jsonl 前两行常为 `mode` / `file-history-snapshot` 事件,**无 cwd、无 timestamp**;
  cwd 从第 2~3 行起的 `attachment`/`user` 等事件才出现,且带 cwd 的事件均带 `timestamp`。
  →「读首行跳过」不可行;改为「解析后按 digest.cwd 过滤」(最简、零歧义)。
- 构建/校验命令:前端 `npm run check` / `npm run build`;后端 `cargo test`(文件已有
  `#[cfg(test)]`)、`cargo build`。依赖 chrono/serde/dirs 均在,无需新增 crate。

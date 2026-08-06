# Journal - cyy (Part 1)

> AI development session journal
> Started: 2026-07-09

---



## Session 1: 采集路径过滤(黑白名单)

**Date**: 2026-07-10
**Task**: 采集路径过滤(黑白名单)
**Branch**: `main`

### Summary

为 Claude Code 采集新增路径过滤:基于真实 cwd 的组件级前缀匹配(子目录继承、大小写/分隔符归一、排除优先),CollectConfig 加 includePaths/excludePaths(Rust+TS,向后兼容),collect_conversations 命令增 filter 参数,设置页加「路径过滤」UI;新增 session_allowed/norm 纯函数+9 单测;沉淀 backend collector-spec。拆两个 commit:路径过滤功能 + app.css 有序列表修复。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `adc0c3e` | (see git log) |
| `8671ff0` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: 引入 SQLite 重构数据持久层

**Date**: 2026-07-11
**Task**: 引入 SQLite 重构数据持久层
**Branch**: `main`

### Summary

数据持久层从 tauri-plugin-store 迁移至 SQLite(rusqlite bundled)。配置与历史解耦:AppConfig 移除 history 字段,历史改细粒度 list/add/remove_history 命令,generate_report 返回 HistoryItem。setup 钩子一次性幂等迁移旧 data.json(保留原文件回退)。新增后端 26 项 cargo test 与前端 9 项 vitest(jsdom),固化 backend/storage-spec.md,同步 CLAUDE.md。手动回归全部通过。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `be0f7b3` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: 周报生成(区间采集 + map-reduce 摘要)

**Date**: 2026-08-06
**Task**: 周报生成(区间采集 + map-reduce 摘要)
**Branch**: `main`

### Summary

新增周报功能:区间采集命令 collect_conversations_range(逐日单日切片,collector trait 不变)+ generate_weekly_report(map-reduce:逐日摘要重试3次指数退避失败跳过标注缺失→reduce 整周凝练流式汇总→落库历史)。StreamChunk 加 progress 变体;周报模板可配置(weeklyMapTemplate/weeklyReduceTemplate,settings 双模板区,空配置 Rust 内嵌兜底);新路由 /weekly(本周一~今天默认区间+可选补充要点+单日token预警+进度条);输出/导出面板抽共享组件 ReportPanel 日报复用。同步 collector-spec/storage-spec。cargo test 66 通过,pnpm check 0 错误 0 警告。trellis-check 验证 AC1-AC9 全 PASS。

### Git Commits

| Hash | Message |
|------|---------|
| `9f72dc8` | (see git log) |

### Status

[OK] **Completed**

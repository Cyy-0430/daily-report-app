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

# Design — opencode 采集器

## 决策

### D1: 复用 ZCode 纯函数,不复制粘贴
opencode 与 ZCode 的 SQLite schema 高度同构(同三表、同 part.data.type 枚举),差异只在
路径/排序列/字段命名/有无 parent_id。把 ZCode 的 `build_day_lines` / `extract_from_parts` /
`ms_to_local` / `date_matches` 提升为 `pub(super)`,opencode 直接 `use`。新代码仅 `collect` +
`db_path`(~150 行),其余复用。

### D2: camelCase 兼容走"字段回退链两套并列",而非参数化
opencode 的 `state.input` 用 `filePath`(camelCase),ZCode 用 `file_path`(snake_case)。
方案:在 `extract_from_parts` 的 key 回退链里 `file_path` 之后并列插入 `filePath`,其余
command/pattern/url/description 两套恰好同名。ZCode 数据只命中 snake_case 分支,camelCase
键在 ZCode 数据里不存在不会误命中——零回归。比"传 bool 选大小写"更简单、更不易错。

### D3: db_path 优先 `$HOME/.local/share`,XDG 作回退
实测本机(Windows)opencode db 落在 `C:\Users\CYY\.local\share\opencode\opencode.db`,
**不是** `%LOCALAPPDATA%\opencode`(即 `dirs::data_local_dir()` 给的路径)。opencode 源码
跨平台硬编码 `$HOME/.local/share`。故 db_path 优先此路径并 `.exists()` 判定,XDG 兜底,
都不存在返回主路径(让调用方的 `!exists` 静默跳过生效)。

### D4: 无 sequence 列 → ORDER BY time_created
ZCode 用 `ORDER BY sequence`;opencode 表无此列。实测 `part.time_created` 在同一 message
内单调递增,可作可靠排序键。message 表同理。

### D5: 全采,无 parent_id 过滤
opencode 无 subagent 概念(无 parent_id 列),session 查询不加 WHERE 子句,全部采集。
(若未来 opencode 引入 subagent,再按真实 schema 调整。)

## 数据流

```
collect(date, filter)
  ├─ db_path() → ~/.local/share/opencode/opencode.db (存在性优先)
  ├─ Connection::open_with_flags(READ_ONLY | NO_MUTEX)  // 失败静默跳过
  ├─ SELECT id, directory, title FROM session            // 全采,无 parent_id 过滤
  └─ per session:
       ├─ session_allowed(directory, includes, excludes) // 真实 cwd 路径过滤,排除优先
       ├─ SELECT id, time_created, data FROM message
       │    WHERE session_id=? ORDER BY time_created     // 无 sequence
       ├─ per message: SELECT data FROM part
       │    WHERE message_id=? ORDER BY time_created     // 无 sequence
       └─ build_day_lines(&day_input, date)              // 复用 zcode:跨天切片 + extract_from_parts
            └─ extract_from_parts(parts)                 // 复用 zcode:text/tool/reasoning/patch 过滤
                 └─ tool key 回退链含 camelCase(filePath) // D2
```

## 关键差异表(opencode vs ZCode)

| 维度 | ZCode | opencode | 影响 |
|---|---|---|---|
| db 路径 | `~/.zcode/cli/db/db.sqlite` | `~/.local/share/opencode/opencode.db` | 新 `db_path()` |
| 主/sub | `parent_id IS NULL` | 无 parent_id,全采 | 去掉 WHERE |
| 排序 | `ORDER BY sequence` | `ORDER BY time_created` | SQL 改 |
| 工具参数 | snake_case | camelCase | `extract_from_parts` 回退链 |
| 额外 part | `file` 丢弃 | `file` + `patch` 丢弃 | `_ => {}` 天然覆盖 |

## 验证状态

- 单测:9 个(含 1 ignored 真实库),全绿。重点钉 camelCase input、patch 丢弃、跨天切片。
- 真实库端到端(2026-08-03):2 sessions / 7+16 lines,cwd=`D:/hand/yqnf/yqnf-contract`,
  project 取 title,tools 正确提取——通过。
- `cargo test --lib`:64 passed / 0 failed / 0 warning。
- `pnpm build`:通过(TS 类型无误)。
- ZCode 回归:`extract_tool_part_camelcase_input` 新增断言钉死兼容扩展不回归 snake_case。

## 风险

- opencode schema 未来变更(如引入 sequence/parent_id):目前按实测 schema 实现,spec 已记录,
  变更时需回归。`patch` part 若未来结构变化,`_ => {}` 仍安全丢弃。

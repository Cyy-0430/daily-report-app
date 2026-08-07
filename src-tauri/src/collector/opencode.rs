//! opencode(sst/opencode)对话采集器。
//!
//! 数据源:`~/.local/share/opencode/opencode.db`(SQLite,遵循 XDG)。
//! 与 ZCode 同构:`session` + `message` + `part` 三表;但 opencode **没有 subagent
//! 概念**(无 `parent_id`),所有 session 都是用户直接交互,全部采集。
//!
//! 三张核心表:
//! - `session`:`id` / `directory`(真实 cwd) / `title` / `time_created`(Unix 毫秒)。
//! - `message`:`session_id` / `time_created`(Unix 毫秒) / `data`(`{role,...}`)。
//! - `part`:`message_id` / `data`(`{type:text|tool|reasoning|step-start|step-finish|patch,...}`)。
//!
//! - **时间过滤(硬契约)**:按 `message.time_created`(毫秒)转本地时区后比 date,
//!   绝不按文件修改时间——session 跨天累积(同 session 按目标日切片)。
//! - **排序键**:opencode 的 message/part 表**没有 `sequence` 列**(与 ZCode 不同),
//!   一律按 `time_created` 升序(实测同 message 内单调递增)。
//! - **字段过滤(策略①)**:复用 [`zcode::extract_from_parts`],保留 user/assistant 文本
//!   (`part.type=text`)+ 工具调用(`part.type=tool`);丢弃 `reasoning`/`step-*`/
//!   `patch`/`file`。工具参数 key 回退链同时兼容 snake_case 与 camelCase——
//!   opencode 的 `state.input` 用 camelCase(`filePath`/`command`/`pattern`)。
//! - **只读访问**:以 `READ_ONLY | NO_MUTEX` 打开 db,opencode 运行写入时不阻塞;
//!   db 不存在或打开失败一律安静跳过,不阻断其它采集器。

use super::claude_code::session_allowed;
#[cfg(test)]
use super::zcode::extract_from_parts;
use super::zcode::{build_day_lines, ms_to_local};
use super::{session_tokens, Collector, PathFilter, SessionDigest};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct OpencodeCollector;

impl Collector for OpencodeCollector {
    fn id(&self) -> &'static str {
        super::TOOL_ID_OPENCODE
    }
    fn display_name(&self) -> &'static str {
        "Opencode"
    }

    fn default_path(&self) -> Option<PathBuf> {
        db_path()
    }

    fn collect(
        &self,
        date: NaiveDate,
        filter: &PathFilter,
        custom_path: Option<&str>,
    ) -> Result<(Vec<SessionDigest>, usize), String> {
        // 非空覆盖 → 展开后用之;否则用默认;两者皆空 → 静默跳过。
        let Some(db) = custom_path
            .and_then(super::expand_home)
            .or_else(|| self.default_path())
        else {
            return Ok((Vec::new(), 0)); // 无法定位主目录 → 静默跳过
        };
        if !db.exists() {
            return Ok((Vec::new(), 0)); // opencode 未安装 → 静默跳过
        }
        // 只读打开:避免锁住正在写入的 opencode 进程;失败也静默跳过。
        let conn = match Connection::open_with_flags(
            &db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => return Ok((Vec::new(), 0)),
        };

        let mut digests = Vec::new();
        let mut skipped = 0usize;

        // 全部 session(opencode 无 subagent,无 parent_id 过滤)。
        let sessions: Vec<(String, Option<String>, Option<String>)> = {
            let Ok(mut stmt) = conn.prepare("SELECT id, directory, title FROM session") else {
                return Ok((Vec::new(), 0));
            };
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .ok()
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_default()
        };

        for (sid, directory, title) in sessions {
            // 路径过滤(基于真实 cwd=directory,组件级前缀匹配,排除优先)。
            let cwd_path = directory.as_deref().map(Path::new);
            if !session_allowed(cwd_path, &filter.includes, &filter.excludes) {
                continue;
            }

            // 取该 session 全部 message(按 time_created),再逐条取其 part,
            // 组装成 (time_ms, message_data, parts) 交给纯函数 build_day_lines 做日期切片。
            // (opencode 无 sequence 列,改用 time_created 排序——见模块注释。)
            let msgs_raw: Vec<(String, i64, String)> = {
                let Ok(mut stmt) = conn.prepare(
                    "SELECT id, time_created, data FROM message WHERE session_id = ? ORDER BY time_created",
                ) else {
                    continue;
                };
                stmt.query_map(params![&sid], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .ok()
                .map(|rows| rows.filter_map(|x| x.ok()).collect())
                .unwrap_or_default()
            };

            let mut day_input: Vec<(i64, Value, Vec<Value>)> = Vec::new();
            for (mid, tms, mdata) in &msgs_raw {
                let data: Value = serde_json::from_str(mdata).unwrap_or(Value::Null);
                let parts: Vec<Value> = match conn
                    .prepare("SELECT data FROM part WHERE message_id = ? ORDER BY time_created")
                {
                    Ok(mut stmt) => match stmt.query_map(params![mid], |r| r.get::<_, String>(0)) {
                        Ok(rows) => {
                            let mut out = Vec::new();
                            for s in rows.flatten() {
                                match serde_json::from_str::<Value>(&s) {
                                    Ok(v) => out.push(v),
                                    Err(_) => skipped += 1, // part JSON 损坏 → 计健康度
                                }
                            }
                            out
                        }
                        Err(_) => Vec::new(),
                    },
                    Err(_) => Vec::new(),
                };
                day_input.push((*tms, data, parts));
            }

            let (lines, started_ms, ended_ms) = build_day_lines(&day_input, date);
            if lines.is_empty() {
                continue;
            }

            let project = title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .or_else(|| {
                    directory.as_deref().and_then(|d| {
                        Path::new(d)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                    })
                })
                .unwrap_or_else(|| sid.clone());

            let started_at = started_ms
                .and_then(ms_to_local)
                .map(|t| t.format(super::FMT_DATE_HM).to_string())
                .unwrap_or_default();
            let ended_at = ended_ms
                .and_then(ms_to_local)
                .map(|t| t.format(super::FMT_HM).to_string())
                .unwrap_or_default();

            let line_count = lines.len();
            let est_tokens = session_tokens(&lines);
            digests.push(SessionDigest {
                tool: self.display_name().to_string(),
                project,
                cwd: directory,
                session_id: sid,
                started_at,
                ended_at,
                line_count,
                est_tokens,
                lines,
            });
        }

        digests.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok((digests, skipped))
    }
}

/// `~/.local/share/opencode/opencode.db`。opencode 跨平台统一用 Unix 风格的
/// `$HOME/.local/share`(实测 Windows 也落在此处,**不**走 `%LOCALAPPDATA%`)。
/// 优先以此路径定位;`data_local_dir`(XDG)仅作回退兜底(个别 Linux 环境可能用 XDG)。
/// 无法定位主目录返回 None。
fn db_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let primary = home
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db");
    if primary.exists() {
        return Some(primary);
    }
    // 回退:XDG 数据目录(部分 Linux 桌面 / 自定义 XDG_DATA_HOME 时更准确)。
    if let Some(dir) = dirs::data_local_dir() {
        let alt = dir.join("opencode").join("opencode.db");
        if alt.exists() {
            return Some(alt);
        }
    }
    // 都不存在时返回主路径(让调用方的 !exists 静默跳过逻辑生效)。
    Some(primary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::Role;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    /// 策略①:text part → 文本保留(复用 zcode::extract_from_parts,结构等价验证)。
    #[test]
    fn extract_text_part() {
        let parts = vec![v(r#"{"type":"text","text":"提交并推送完成"}"#)];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(text, "提交并推送完成");
        assert!(tools.is_empty());
    }

    /// 策略①(opencode 关键差异):tool part 的 input 为 **camelCase**(`filePath`)。
    #[test]
    fn extract_tool_part_camelcase_filepath() {
        let parts = vec![v(
            r#"{"type":"tool","tool":"read","state":{"status":"completed",
            "input":{"filePath":"D:/hand/yqnf/yqnf-contract/src/main.rs"}}}"#,
        )];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert!(text.is_empty());
        assert_eq!(tools, vec!["read: D:/hand/yqnf/yqnf-contract/src/main.rs"]);
    }

    /// 策略①:tool input 为 `command`(bash 工具),camelCase 字段同样命中。
    #[test]
    fn extract_tool_part_camelcase_command() {
        let parts = vec![v(
            r#"{"type":"tool","tool":"bash","state":{"status":"completed",
            "input":{"command":"git rev-parse --abbrev-ref HEAD","description":"get branch"}}}"#,
        )];
        let (_, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(tools, vec!["bash: git rev-parse --abbrev-ref HEAD"]);
    }

    /// 策略①:`patch`(opencode 特有 part,代码补丁)被丢弃。
    #[test]
    fn extract_skips_patch() {
        let parts = vec![v(
            r#"{"type":"patch","path":"src/a.ts","content":"--- a\n+++ b\n@@ ..."}"#,
        )];
        assert!(extract_from_parts(&parts).is_none());
    }

    /// 策略①:`reasoning` / `step-start` / `step-finish` 被丢弃。
    #[test]
    fn extract_skips_reasoning_and_step() {
        let parts = vec![
            v(r#"{"type":"step-start"}"#),
            v(r#"{"type":"reasoning","text":"用户想让我提交…"}"#),
            v(r#"{"type":"step-finish","reason":"tool-calls"}"#),
        ];
        assert!(extract_from_parts(&parts).is_none());
    }

    /// 策略①:混合——留 text + tool(camelCase input),丢 reasoning / step / patch。
    #[test]
    fn extract_mixed_keeps_text_and_tool() {
        let parts = vec![
            v(r#"{"type":"step-start"}"#),
            v(r#"{"type":"reasoning","text":"先查当前分支"}"#),
            v(r#"{"type":"text","text":"我先确认当前分支"}"#),
            v(r#"{"type":"tool","tool":"bash","state":{"input":{"command":"git status"}}}"#),
            v(r#"{"type":"step-finish"}"#),
            v(r#"{"type":"patch","path":"a.ts","content":"..."}"#),
        ];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(text, "我先确认当前分支");
        assert_eq!(tools, vec!["bash: git status"]);
    }

    /// build_day_lines 复用(跨天切片):opencode 真实时间戳风格(2026-08-03 北京时间)。
    /// 1785750244447 ms ≈ 2026-08-03 16:50 +08:00;1785750275013 ms ≈ 同日稍后。
    #[test]
    fn build_day_lines_cross_day_slice() {
        // 16:49 本地(+08)vs 16:49 UTC(次日 00:49 本地)。
        let d0803 = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        let msgs = vec![
            (
                1785663846000, // 2026-08-02 ~16:24 UTC → 08-03 00:24 本地
                v(r#"{"role":"user"}"#),
                vec![v(r#"{"type":"text","text":"前一天的输入"}"#)],
            ),
            (
                1785750244447, // 2026-08-03 本地
                v(r#"{"role":"assistant"}"#),
                vec![v(r#"{"type":"text","text":"当天的回复"}"#)],
            ),
        ];
        let (lines, started, ended) = build_day_lines(&msgs, d0803);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "当天的回复");
        assert!(matches!(lines[0].role, Role::Assistant));
        assert_eq!(started, Some(1785750244447));
        assert_eq!(ended, Some(1785750244447));
    }

    /// build_day_lines:目标日期无 message → 空行,None 时间。
    #[test]
    fn build_day_lines_no_match_is_empty() {
        let d0701 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let msgs = vec![(
            1785750244447,
            v(r#"{"role":"user"}"#),
            vec![v(r#"{"type":"text","text":"x"}"#)],
        )];
        let (lines, started, ended) = build_day_lines(&msgs, d0701);
        assert!(lines.is_empty());
        assert_eq!(started, None);
        assert_eq!(ended, None);
    }

    /// db_path:能定位到 opencode.db(文件名固定;父目录含 opencode 段)。
    /// 本机已安装 opencode,期望命中真实存在的 `~/.local/share/opencode/opencode.db`。
    #[test]
    fn db_path_points_to_opencode_db() {
        let p = db_path().expect("应能定位主目录");
        assert!(
            p.ends_with("opencode.db"),
            "文件名应为 opencode.db: {:?}",
            p
        );
        // 父目录以 opencode 收尾(无论 .local/share/opencode 还是 XDG .../opencode)。
        let parent = p.parent().unwrap();
        assert!(
            parent.ends_with("opencode"),
            "应在 opencode 目录下,父目录为 {:?}",
            parent
        );
    }

    /// 端到端:对真实 `~/.local/share/opencode/opencode.db` 采集 2026-08-03
    /// (已知有 yqnf-contract 提交会话)。需本机装过 opencode,默认 ignored,手动跑:
    /// `cargo test --manifest-path src-tauri/Cargo.toml collect_real_opencode_sample_day -- --ignored --nocapture`
    #[test]
    #[ignore = "需要本地 opencode db,仅手动验证"]
    fn collect_real_opencode_sample_day() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        let (digests, skipped) = OpencodeCollector
            .collect(date, &PathFilter::default(), None)
            .expect("采集不应报错");
        println!(
            "== opencode 采集 2026-08-03: sessions={}, skipped={} ==",
            digests.len(),
            skipped
        );
        for d in &digests {
            println!(
                "  [{}] {} | cwd={:?} | {} lines | {} ~ {}",
                d.session_id, d.project, d.cwd, d.line_count, d.started_at, d.ended_at
            );
            for ln in d.lines.iter().take(3) {
                let preview: String = ln.text.chars().take(50).collect();
                println!(
                    "    {} {:?} {} (tools={})",
                    ln.ts,
                    ln.role,
                    preview,
                    ln.tools.len()
                );
            }
        }
        assert!(
            !digests.is_empty(),
            "2026-08-03 应至少采集到一个 opencode 会话"
        );
    }
}

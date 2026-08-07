//! ZCode 对话采集器。
//!
//! 数据源:`~/.zcode/cli/db/db.sqlite`(SQLite)。主会话与子 agent 分表存储,
//! 这里只采**主会话**(`session.parent_id IS NULL`),subagent(`sess_subagent_*`)
//! 的探索过程不进日报。
//!
//! 三张核心表:
//! - `session`:`id` / `parent_id`(NULL=主会话) / `directory`(真实 cwd) /
//!   `title` / `time_created`(Unix 毫秒)。
//! - `message`:`session_id` / `time_created`(Unix 毫秒) / `data`(`{role,...}`)。
//! - `part`:`message_id` / `data`(`{type:text|tool|reasoning|step-*,...}`)。
//!
//! - **时间过滤(硬契约)**:按 `message.time_created`(毫秒)转本地时区后比 date,
//!   绝不按文件修改时间——session 跨天累积(同 session 按目标日切片)。
//! - **字段过滤(策略①)**:保留 user/assistant 文本(`part.type=text`)+ 工具调用
//!   (`part.type=tool` 的 name + 关键参数);丢弃 `reasoning`(= thinking)与
//!   `step-start`/`step-finish`/`file` 等元数据。
//! - **只读访问**:以 `READ_ONLY | NO_MUTEX` 打开 db,ZCode 运行写入时不阻塞;
//!   db 不存在或打开失败一律安静跳过,不阻断其它采集器。

use super::claude_code::session_allowed;
use super::{session_tokens, Collector, ConversationLine, PathFilter, Role, SessionDigest};
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct ZCodeCollector;

impl Collector for ZCodeCollector {
    fn id(&self) -> &'static str {
        super::TOOL_ID_ZCODE
    }
    fn display_name(&self) -> &'static str {
        "ZCode"
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
            return Ok((Vec::new(), 0)); // ZCode 未安装 → 静默跳过
        }
        // 只读打开:避免锁住正在写入的 ZCode 进程;失败也静默跳过。
        let conn = match Connection::open_with_flags(
            &db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => return Ok((Vec::new(), 0)),
        };

        let mut digests = Vec::new();
        let mut skipped = 0usize;

        // 主会话(parent_id IS NULL)。
        let sessions: Vec<(String, Option<String>, Option<String>)> = {
            let Ok(mut stmt) = conn.prepare(
                "SELECT id, directory, title FROM session WHERE parent_id IS NULL",
            ) else {
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

            // 取该 session 全部 message(按 sequence),再逐条取其 part,组装成
            // (time_ms, message_data, parts) 交给纯函数 build_day_lines 做日期切片。
            let msgs_raw: Vec<(String, i64, String)> = {
                let Ok(mut stmt) = conn.prepare(
                    "SELECT id, time_created, data FROM message WHERE session_id = ? ORDER BY sequence",
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
                    .prepare("SELECT data FROM part WHERE message_id = ? ORDER BY sequence")
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
                    directory
                        .as_deref()
                        .and_then(|d| Path::new(d).file_name().map(|n| n.to_string_lossy().into_owned()))
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

/// `~/.zcode/cli/db/db.sqlite`。
fn db_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".zcode").join("cli").join("db").join("db.sqlite"))
}

/// Unix 毫秒 → 本地时区时间。无效值返回 None。
pub(super) fn ms_to_local(ms: i64) -> Option<DateTime<Local>> {
    Local.timestamp_millis_opt(ms).single()
}

/// 该毫秒是否落在目标本地日期(硬契约:按消息时间,非文件 mtime)。
pub(super) fn date_matches(ms: i64, date: NaiveDate) -> bool {
    ms_to_local(ms)
        .map(|t| t.date_naive() == date)
        .unwrap_or(false)
}

/// 从一个 session 的全部 message 行(已按时间顺序取齐 parts)构建**目标日期**
/// 的对话行,并返回当天首/末毫秒。纯函数,便于单测跨天切片与时间过滤。
///
/// - 非目标日期的 message 整条跳过(不计入 lines);
/// - 每条 message 的 role 取自 `data.role`,内容取自其 parts(策略①);
/// - 无有效文本且无工具调用的 message 跳过。
///
/// `pub(super)` 供同构的 opencode 采集器复用(ZCode 按 `sequence` 排、opencode 按
/// `time_created` 排,但进入此函数时 parts 已是顺序数组,函数本身与排序键无关)。
pub(super) fn build_day_lines(
    msgs: &[(i64, Value, Vec<Value>)],
    date: NaiveDate,
) -> (Vec<ConversationLine>, Option<i64>, Option<i64>) {
    let mut lines = Vec::new();
    let mut started: Option<i64> = None;
    let mut ended: Option<i64> = None;
    for (tms, data, parts) in msgs {
        if !date_matches(*tms, date) {
            continue; // 非目标日期:跨天切片
        }
        let role = match data["role"].as_str() {
            Some("assistant") => Role::Assistant,
            _ => Role::User,
        };
        let Some((text, tools)) = extract_from_parts(parts) else {
            continue;
        };
        started = Some(started.map_or(*tms, |s| s.min(*tms)));
        ended = Some(ended.map_or(*tms, |e| e.max(*tms)));
        let ts = ms_to_local(*tms)
            .map(|t| t.format(super::FMT_HM).to_string())
            .unwrap_or_default();
        lines.push(ConversationLine {
            ts,
            role,
            text,
            tools,
        });
    }
    (lines, started, ended)
}

/// 策略①字段过滤:从一条 message 的 parts 提取 (文本, 工具摘要)。
/// 无有效内容返回 None。丢弃 `reasoning`/`step-*`/`file`/`patch`;`tool` 仅留 name + 关键参数。
///
/// `pub(super)` 供同构的 opencode 采集器复用。工具参数 key 回退链**同时兼容
/// snake_case 与 camelCase**:ZCode 用 `file_path`,opencode 用 `filePath`;
/// 两套都列出,实际命中哪套取决于数据源,互不干扰。
pub(super) fn extract_from_parts(parts: &[Value]) -> Option<(String, Vec<String>)> {
    let mut texts = Vec::new();
    let mut tools = Vec::new();
    for p in parts {
        match p["type"].as_str().unwrap_or("") {
            "text" => {
                if let Some(t) = p["text"].as_str() {
                    let t = t.trim();
                    if !t.is_empty() {
                        texts.push(t.to_string());
                    }
                }
            }
            "tool" => {
                let name = p["tool"].as_str().unwrap_or("tool");
                let inp = &p["state"]["input"];
                let key = inp["file_path"]
                    .as_str()
                    .or_else(|| inp["filePath"].as_str())
                    .or_else(|| inp["path"].as_str())
                    .or_else(|| inp["command"].as_str())
                    .or_else(|| inp["pattern"].as_str())
                    .or_else(|| inp["url"].as_str())
                    .or_else(|| inp["description"].as_str())
                    .unwrap_or("");
                let key = super::truncate(key, super::TOOL_KEY_MAX_LEN);
                tools.push(if key.is_empty() {
                    name.to_string()
                } else {
                    format!("{name}: {key}")
                });
            }
            _ => {} // reasoning / step-start / step-finish / file / patch → 丢弃
        }
    }
    if texts.is_empty() && tools.is_empty() {
        return None;
    }
    Some((texts.join("\n"), tools))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    /// 策略①:text part → 文本保留。
    #[test]
    fn extract_text_part() {
        let parts = vec![v(r#"{"type":"text","text":"Cherry-pick 成功完成"}"#)];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(text, "Cherry-pick 成功完成");
        assert!(tools.is_empty());
    }

    /// 策略①:tool part → name + 关键参数(command)。
    #[test]
    fn extract_tool_part() {
        let parts = vec![v(
            r#"{"type":"tool","tool":"Bash","state":{"status":"completed",
            "input":{"command":"git log --oneline","description":"verify"}}}"#,
        )];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert!(text.is_empty());
        assert_eq!(tools, vec!["Bash: git log --oneline"]);
    }

    /// 策略①:reasoning / step-* / file 被丢弃 → 无有效内容返回 None。
    #[test]
    fn extract_skips_reasoning_and_step() {
        let parts = vec![
            v(r#"{"type":"reasoning","text":"内部推理……"}"#),
            v(r#"{"type":"step-start"}"#),
            v(r#"{"type":"step-finish","reason":"stop"}"#),
            v(r#"{"type":"file","path":"a.txt"}"#),
        ];
        assert!(extract_from_parts(&parts).is_none());
    }

    /// 策略①:text + tool + reasoning 混合:留 text 与 tool,丢 reasoning。
    #[test]
    fn extract_mixed_keeps_text_and_tool() {
        let parts = vec![
            v(r#"{"type":"reasoning","text":"推理"}"#),
            v(r#"{"type":"text","text":"好的,我开始"}"#),
            v(r#"{"type":"tool","tool":"Read","state":{"input":{"file_path":"src/a.ts"}}}"#),
        ];
        let (text, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(text, "好的,我开始");
        assert_eq!(tools, vec!["Read: src/a.ts"]);
    }

    /// 策略①:tool input 的 camelCase 字段(`filePath`)同样命中(为 opencode 兼容,
    /// 见函数注释;ZCode 自身用 snake_case,此处钉死兼容回退不会回归)。
    #[test]
    fn extract_tool_part_camelcase_input() {
        let parts = vec![v(
            r#"{"type":"tool","tool":"read","state":{"input":{"filePath":"src/a.ts"}}}"#,
        )];
        let (_, tools) = extract_from_parts(&parts).expect("应有内容");
        assert_eq!(tools, vec!["read: src/a.ts"]);
    }

    /// 时间硬契约:毫秒 → 本地日期判定(隐含跨天:同一 ms 在不同 date 判定不同)。
    /// 1783931753991 在 UTC 与 UTC+8 均落在 2026-07-13,1781504646766 落在 2026-06-15。
    #[test]
    fn date_matches_millisecond_filter() {
        let d0713 = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        let d0615 = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        assert!(date_matches(1783931753991, d0713));
        assert!(!date_matches(1783931753991, d0615));
        assert!(date_matches(1781504646766, d0615));
        assert!(!date_matches(1781504646766, d0713));
    }

    /// 跨天切片:同一 session 的两条 message 分属不同日期,只留目标日期;
    /// started/ended 取当天首/末毫秒。
    #[test]
    fn build_day_lines_cross_day_slice() {
        let d0713 = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        let msgs = vec![
            // 6-15 的消息(非目标)。
            (
                1781504646766,
                v(r#"{"role":"user"}"#),
                vec![v(r#"{"type":"text","text":"六月的那条"}"#)],
            ),
            // 7-13 的消息(目标)。
            (
                1783931753991,
                v(r#"{"role":"assistant"}"#),
                vec![v(r#"{"type":"text","text":"七月的那条"}"#)],
            ),
        ];
        let (lines, started, ended) = build_day_lines(&msgs, d0713);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "七月的那条");
        assert!(matches!(lines[0].role, Role::Assistant));
        assert_eq!(started, Some(1783931753991));
        assert_eq!(ended, Some(1783931753991));
    }

    /// 跨天切片:目标日期无 message → 空行,None 时间。
    #[test]
    fn build_day_lines_no_match_is_empty() {
        let d0701 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let msgs = vec![(
            1783931753991,
            v(r#"{"role":"user"}"#),
            vec![v(r#"{"type":"text","text":"x"}"#)],
        )];
        let (lines, started, ended) = build_day_lines(&msgs, d0701);
        assert!(lines.is_empty());
        assert_eq!(started, None);
        assert_eq!(ended, None);
    }

    /// 端到端:对真实 `~/.zcode/cli/db/db.sqlite` 采集某历史日期(2026-07-13,
    /// 已知有主会话数据)。需本机装过 ZCode,默认 ignored,手动跑:
    /// `cargo test --lib collect_real_zcode_sample_day -- --ignored --nocapture`
    #[test]
    #[ignore = "需要本地 ZCode db,仅手动验证"]
    fn collect_real_zcode_sample_day() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        let (digests, skipped) = ZCodeCollector
            .collect(date, &PathFilter::default(), None)
            .expect("采集不应报错");
        println!("== ZCode 采集 2026-07-13: sessions={}, skipped={} ==", digests.len(), skipped);
        for d in &digests {
            println!(
                "  [{}] {} | cwd={:?} | {} lines | {} ~ {}",
                d.session_id, d.project, d.cwd, d.line_count, d.started_at, d.ended_at
            );
            for ln in d.lines.iter().take(3) {
                let preview: String = ln.text.chars().take(50).collect();
                println!("    {} {:?} {} (tools={})", ln.ts, ln.role, preview, ln.tools.len());
            }
        }
        assert!(!digests.is_empty(), "2026-07-13 应至少采集到一个 ZCode 主会话");
    }
}

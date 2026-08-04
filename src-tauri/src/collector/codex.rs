//! Codex(OpenAI Codex CLI)对话采集器。
//!
//! 数据源:`~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<uuid>.jsonl`,
//! 每个文件一个会话,**每行一个事件 JSON**(append-only)。顶层字段:
//! `timestamp`(UTC, RFC3339)、`type`、`payload`。
//!
//! Codex 的对话有两套并行表示:
//! - `event_msg` 的 `user_message` / `agent_message` —— **TUI 展示层文本**,
//!   干净、无系统噪声(`payload.message`)。本采集器**只用这条**作文本源。
//! - `response_item` 的 `message`(role=developer/user/assistant)—— API 层原始
//!   消息,含注入的权限 / AGENTS.md / skills 指令等噪声,user 的真实输入已由
//!   `user_message` 干净覆盖,故**整类丢弃**(避免重复与噪声)。
//! - `response_item` 的 `function_call` / `local_shell_call` —— 工具调用,渲染为
//!   Assistant 工具行(本机暂无样本,见 `extract_line` 注释)。
//!
//! - **时间过滤(硬契约)**:按每行顶层 `timestamp`(UTC, RFC3339)转本地时区后比
//!   date,绝不按文件名日期段或文件修改时间——rollout 文件按会话**起始**日归档,
//!   跨天延续的会话行需靠 timestamp 才能被目标日命中。
//! - **cwd 来自 `session_meta.payload.cwd`**:真实路径(无 Claude Code 的目录名
//!   编码歧义),直接喂给 `session_allowed`,沿用组件级前缀 / 排除优先语义。
//! - **字段过滤(策略①)**:保留 user/assistant 可见文本 + 工具调用摘要;丢弃
//!   developer 注入消息、`task_*` / `token_count` 等元数据事件;并剥离 `event_msg`
//!   文本里内嵌的工具结果 / 命令回显全文(`[external_agent_tool_result]`、
//!   `<command-*>`、`<task-notification>` 等),见 [`clean_message`]。

use super::claude_code::session_allowed;
use super::{session_tokens, Collector, ConversationLine, PathFilter, Role, SessionDigest};
use chrono::{DateTime, Local, NaiveDate};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CodexCollector;

impl Collector for CodexCollector {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn collect(
        &self,
        date: NaiveDate,
        filter: &PathFilter,
    ) -> Result<(Vec<SessionDigest>, usize), String> {
        let base = home_sessions_dir()?;
        if !base.exists() {
            return Ok((Vec::new(), 0)); // Codex 未安装 → 静默跳过
        }

        // 递归收集所有 rollout *.jsonl(跨目录层级,不按文件名日期剪枝——见模块注释)。
        let mut files = Vec::new();
        collect_jsonl_files(&base, &mut files);

        let mut digests = Vec::new();
        let mut skipped = 0usize;
        for f in files {
            let (digest_opt, sk) = parse_session(&f, self.display_name(), date);
            skipped += sk;
            if let Some(d) = digest_opt {
                if !d.lines.is_empty() {
                    // 路径过滤(基于真实 cwd=session_meta.cwd,组件级前缀,排除优先):
                    // push 前判定,保持 parse_session 单一职责(同 Claude Code)。
                    let cwd_path = d.cwd.as_deref().map(Path::new);
                    if session_allowed(cwd_path, &filter.includes, &filter.excludes) {
                        digests.push(d);
                    }
                }
            }
        }

        digests.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok((digests, skipped))
    }
}

/// `~/.codex/sessions`。无法定位主目录时返回 Err(由调用方决定是否上抛)。
fn home_sessions_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())?;
    Ok(home.join(".codex").join("sessions"))
}

/// 递归收集 `dir` 下所有 `*.jsonl`(Codex 按 `<Y>/<M>/<D>/rollout-*.jsonl` 归档)。
/// 读取失败的子目录安静跳过,不阻断整体采集。
fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_jsonl_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
}

/// 解析单个 rollout jsonl 为 session(仅保留目标日期的行)。
/// 返回 (Option<SessionDigest>, 跳过行数)。
fn parse_session(
    path: &Path,
    tool_name: &str,
    date: NaiveDate,
) -> (Option<SessionDigest>, usize) {
    // session_id 优先取 session_meta.payload.session_id,回退用文件名里的 uuid 段。
    let file_uuid = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|s| s.rsplit_once('-').map(|(_, u)| u.trim_end_matches(".jsonl")))
        .unwrap_or("")
        .to_string();

    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return (None, 0),
    };

    let mut lines: Vec<ConversationLine> = Vec::new();
    let mut cwd: Option<String> = None;
    let mut session_id: Option<String> = None;
    let mut started: Option<String> = None;
    let mut ended: Option<String> = None;
    let mut skipped = 0usize;

    for raw in content.lines() {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let ev: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        // cwd / session_id 仅取自 session_meta(会话级,不随行覆盖)。
        if ev["type"].as_str() == Some("session_meta") {
            if cwd.is_none() {
                if let Some(c) = ev["payload"]["cwd"].as_str() {
                    cwd = Some(c.to_string());
                }
            }
            if session_id.is_none() {
                if let Some(s) = ev["payload"]["session_id"].as_str() {
                    session_id = Some(s.to_string());
                }
            }
        }

        // 时间过滤(硬契约):UTC → 本地时区 → 比 date。
        let Some(ts_str) = ev["timestamp"].as_str() else {
            // session_meta 等无 timestamp 的事件若未消费则不计 skipped(正常过滤)。
            continue;
        };
        let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) else {
            skipped += 1;
            continue;
        };
        let local = dt.with_timezone(&Local);
        if local.date_naive() != date {
            continue; // 非目标日期:跨天切片,不计入 skipped
        }
        let ts_disp = local.format("%H:%M").to_string();

        if started.is_none() {
            started = Some(local.format("%Y-%m-%d %H:%M").to_string());
        }
        ended = Some(local.format("%H:%M").to_string());

        if let Some((role, text, tools)) = extract_line(&ev) {
            lines.push(ConversationLine {
                ts: ts_disp,
                role,
                text,
                tools,
            });
        }
    }

    if lines.is_empty() {
        return (None, skipped);
    }
    let line_count = lines.len();
    let est_tokens = session_tokens(&lines);
    let session_id = session_id.unwrap_or_else(|| file_uuid.clone());
    let project = project_name(cwd.as_deref(), &session_id);
    let digest = SessionDigest {
        tool: tool_name.to_string(),
        project,
        cwd,
        session_id,
        started_at: started.unwrap_or_default(),
        ended_at: ended.unwrap_or_default(),
        line_count,
        est_tokens,
        lines,
    };
    (Some(digest), skipped)
}

/// project 显示名:cwd 的 basename(如 `yqnf-contract`);不可得则回退 session_id 前 8 位。
fn project_name(cwd: Option<&str>, session_id: &str) -> String {
    if let Some(c) = cwd {
        if let Some(base) = Path::new(c).file_name().and_then(|n| n.to_str()) {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    session_id.chars().take(8).collect()
}

/// 策略①字段过滤:从一条 rollout 事件提取 (角色, 文本, 工具摘要)。无有效内容返回 None。
///
/// 文本源:`event_msg` 的 `user_message` / `agent_message`(`payload.message`),
/// 经 [`clean_message`] 剥离内嵌的工具结果 / 命令回显等全文(策略①)。
/// 工具源(防御性,本机零样本):`response_item` 的 `function_call` /
/// `local_shell_call`。`response_item` 的 `message`(含 developer 注入噪声)整类丢弃。
fn extract_line(ev: &Value) -> Option<(Role, String, Vec<String>)> {
    let ty = ev["type"].as_str()?;
    let p = &ev["payload"];
    match ty {
        "event_msg" => match p["type"].as_str()? {
            "user_message" => cleaned_line(p["message"].as_str(), Role::User),
            "agent_message" => cleaned_line(p["message"].as_str(), Role::Assistant),
            _ => None, // task_started / task_complete / token_count / ... → 丢弃
        },
        "response_item" => match p["type"].as_str()? {
            // 工具调用(防御性:本机无样本,按公开 rollout schema 实现 + 合成单测钉结构)。
            "function_call" => {
                let name = p["name"].as_str().unwrap_or("tool");
                let key = parse_tool_key(&p["arguments"]);
                Some(tool_only(name, key))
            }
            "local_shell_call" => {
                let key = p["action"]["command"].as_str().map(|s| truncate(s, 80));
                Some(tool_only("shell", key))
            }
            // message(developer/user/assistant 原始 API 消息)→ 整类丢弃(噪声 + 重复)。
            _ => None,
        },
        _ => None, // session_meta / world_state / turn_context / ... → 丢弃
    }
}

/// 文本行:对 `message` 跑 [`clean_message`] 剥离内嵌块;文本与工具皆空 → None。
fn cleaned_line(message: Option<&str>, role: Role) -> Option<(Role, String, Vec<String>)> {
    let (text, tools) = clean_message(message?);
    if text.is_empty() && tools.is_empty() {
        return None;
    }
    Some((role, text, tools))
}

/// 策略①(Codex 扁平文本版):剥掉 `message` 里内嵌的工具结果 / 命令回显全文,
/// 并从工具调用块提取工具名摘要。返回 (幸存文本, 工具摘要列表)。
///
/// Codex 把 tool_result / 命令输出摊平成文本标签塞进 `event_msg` 的 message,
/// 与 Claude Code 的独立 tool_result block 不同——这里手动剥除以等价实现策略①
/// (丢 tool_result 全文,留工具名)。实测可削减 ~82% 字符量。
///
/// 剥除的块(均 open/close 配对):
/// - `[external_agent_tool_result] … [/external_agent_tool_result]` → 丢(= tool_result 全文)
/// - `[external_agent_tool_call[: NAME]] … [/external_agent_tool_call]` → 留 NAME 摘要
/// - `<task-notification> … </task-notification>` → 丢(subagent 通知噪声)
/// - `<command-name>` / `<command-message>` / `<command-args>` / `<local-command-stdout>`
///   → 丢(slash 命令回显及其输出)
///
/// 未找到闭合标签时,把开标签当普通文本保留并继续(容错,不吞后续内容)。
fn clean_message(msg: &str) -> (String, Vec<String>) {
    /// 一个可识别的块:开标记前缀、闭标记、是否提取工具名。
    struct Block(&'static str, &'static str, bool);
    const BLOCKS: &[Block] = &[
        Block("[external_agent_tool_result]", "[/external_agent_tool_result]", false),
        // call 开标记是前缀(后接 `: NAME]` 或 `]`),闭标记固定。
        Block("[external_agent_tool_call", "[/external_agent_tool_call]", true),
        Block("<task-notification>", "</task-notification>", false),
        Block("<command-name>", "</command-name>", false),
        Block("<command-message>", "</command-message>", false),
        Block("<command-args>", "</command-args>", false),
        Block("<local-command-stdout>", "</local-command-stdout>", false),
    ];

    let mut text = String::with_capacity(msg.len());
    let mut tools = Vec::new();
    let mut cursor = 0usize;
    while cursor < msg.len() {
        // 在 cursor 及之后找最早出现的开标记。
        let earliest: Option<(usize, &Block)> = BLOCKS.iter().filter_map(|b| {
            msg[cursor..].find(b.0).map(|off| (cursor + off, b))
        }).min_by_key(|(pos, _)| *pos);

        let Some((start, block)) = earliest else {
            text.push_str(&msg[cursor..]);
            break;
        };
        // 先把开标记之前的正文拷出。
        text.push_str(&msg[cursor..start]);

        // 算出开标记结束位置。call 的开标记以 `]` 收尾,需向后找;其余开标记是固定字面量。
        let open_end = if block.1 == "[/external_agent_tool_call]" {
            // 在 start 之后找第一个 `]`(开标记的收尾)。
            match msg[start..].find(']') {
                Some(rel) => start + rel + 1,
                None => {
                    // 无 `]`:开标记残缺,当普通文本保留并前进一字节组(避免死循环)。
                    text.push_str(&msg[start..start + msg[start..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)]);
                    cursor = start + msg[start..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                    continue;
                }
            }
        } else {
            start + block.0.len()
        };

        // 提取工具名(call 块):开标记内 `: NAME` 的 NAME。
        if block.2 {
            let inner_open = &msg[start + block.0.len() .. open_end.saturating_sub(1)];
            let name = inner_open.trim_start_matches(':').trim();
            if !name.is_empty() {
                tools.push(truncate(name, 80));
            }
        }

        // 找闭标记;找不到则保留开标记字面量、跳过它继续。
        match msg[open_end..].find(block.1) {
            Some(rel) => {
                cursor = open_end + rel + block.1.len();
            }
            None => {
                text.push_str(&msg[start..open_end]);
                cursor = open_end;
            }
        }
    }
    (text.trim().to_string(), tools)
}

/// 工具调用行:name 必有;key 可空(空则仅留 name)。始终产出 Assistant 工具行。
fn tool_only(name: &str, key: Option<String>) -> (Role, String, Vec<String>) {
    let tools = vec![match key {
        Some(k) if !k.is_empty() => format!("{name}: {k}"),
        _ => name.to_string(),
    }];
    (Role::Assistant, String::new(), tools)
}

/// function_call.arguments 是 **JSON 编码的字符串**(如 `"{\"command\":\"...\"}"`),
/// 解析后取回退链首个非空值,截断 80。解析失败返回 None。
fn parse_tool_key(arguments: &Value) -> Option<String> {
    // arguments 通常是字符串,偶尔回退为对象。
    let obj: Value = if let Some(s) = arguments.as_str() {
        serde_json::from_str(s).unwrap_or(Value::Null)
    } else if arguments.is_object() {
        arguments.clone()
    } else {
        Value::Null
    };
    if obj.is_null() {
        return None;
    }
    let key = obj["file_path"]
        .as_str()
        .or_else(|| obj["path"].as_str())
        .or_else(|| obj["command"].as_str())
        .or_else(|| obj["pattern"].as_str())
        .or_else(|| obj["url"].as_str())
        .or_else(|| obj["description"].as_str())?;
    Some(truncate(key, 80))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    /// 策略①:event_msg user_message → User 文本。
    #[test]
    fn extract_user_message() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"user_message","message":"帮我重构登录"}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::User));
        assert_eq!(text, "帮我重构登录");
        assert!(tools.is_empty());
    }

    /// 策略①:event_msg agent_message → Assistant 文本。
    #[test]
    fn extract_agent_message() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"agent_message","message":"好的，开始重构","phase":"final_answer"}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert_eq!(text, "好的，开始重构");
        assert!(tools.is_empty());
    }

    /// 策略①:空白 message → None(跳过)。
    #[test]
    fn extract_blank_message_skipped() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"user_message","message":"   "}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:task_started / token_count 等元数据事件 → None。
    #[test]
    fn extract_skips_metadata_events() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"task_started","turn_id":"t1"}}"#);
        assert!(extract_line(&ev).is_none());
        let ev = v(r#"{"type":"event_msg","payload":{"type":"token_count","input":100}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:response_item/message(developer 注入噪声)整类丢弃 → None。
    #[test]
    fn extract_skips_response_item_message() {
        let ev = v(r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<permissions>…"}]}}"#);
        assert!(extract_line(&ev).is_none());
        // user/assistant 原始 API 消息同样丢弃(已由 event_msg 干净覆盖)。
        let ev = v(r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"x"}]}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:function_call → Assistant 工具行(本机无样本,合成 fixture 钉结构)。
    #[test]
    fn extract_function_call() {
        // arguments 是 JSON 编码字符串。
        let ev = v(r#"{"type":"response_item","payload":{"type":"function_call","name":"Read","arguments":"{\"file_path\":\"src/a.ts\"}"}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert!(text.is_empty());
        assert_eq!(tools, vec!["Read: src/a.ts"]);
    }

    /// function_call:command 参数回退。
    #[test]
    fn extract_function_call_command() {
        let ev = v(r#"{"type":"response_item","payload":{"type":"function_call","name":"Bash","arguments":"{\"command\":\"git log\"}"}}"#);
        let (_, _, tools) = extract_line(&ev).expect("应有内容");
        assert_eq!(tools, vec!["Bash: git log"]);
    }

    /// function_call:arguments 为对象(非常规但容错)。
    #[test]
    fn extract_function_call_object_args() {
        let ev = v(r#"{"type":"response_item","payload":{"type":"function_call","name":"Read","arguments":{"file_path":"x.ts"}}}"#);
        let (_, _, tools) = extract_line(&ev).expect("应有内容");
        assert_eq!(tools, vec!["Read: x.ts"]);
    }

    /// local_shell_call → "shell: <command>"(本机无样本,合成)。
    #[test]
    fn extract_local_shell_call() {
        let ev = v(r#"{"type":"response_item","payload":{"type":"local_shell_call","action":{"command":"ls -la"}}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert!(text.is_empty());
        assert_eq!(tools, vec!["shell: ls -la"]);
    }

    /// session_meta / world_state / turn_context → None。
    #[test]
    fn extract_skips_non_msg_types() {
        let ev = v(r#"{"type":"session_meta","payload":{"cwd":"D:\\x","session_id":"s1"}}"#);
        assert!(extract_line(&ev).is_none());
        let ev = v(r#"{"type":"world_state","payload":{}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:剥离内嵌的 `[external_agent_tool_result]` 全文(= tool_result),只留正文。
    #[test]
    fn clean_strips_tool_result_block() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"agent_message","message":"我看一下文件\n[external_agent_tool_result] 1\tpackage main\n2\tfunc main(){}\n[/external_agent_tool_result]\n找到了问题"}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert!(!text.contains("package main"));
        assert!(text.contains("我看一下文件"));
        assert!(text.contains("找到了问题"));
        assert!(tools.is_empty());
    }

    /// 策略①:`[external_agent_tool_call: NAME]` → 提取 NAME 为工具摘要;正文保留。
    #[test]
    fn clean_extracts_tool_call_name() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"agent_message","message":"开始\n[external_agent_tool_call: Bash]\ninput: {\"command\":\"git log\"}\n[/external_agent_tool_call]\n[external_agent_tool_result]commit abc\n[/external_agent_tool_result]完成"}}"#);
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert!(!text.contains("git log"));
        assert!(text.contains("开始"));
        assert!(text.contains("完成"));
        assert_eq!(tools, vec!["Bash"]);
    }

    /// 策略①:agent_message 整条都是 tool_result → 剥光后无内容 → None。
    #[test]
    fn clean_entirely_result_is_none() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"agent_message","message":"[external_agent_tool_result]一大段文件全文……\n多行\n[/external_agent_tool_result]"}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:slash 命令回显(`<command-*>`)被剥除;剥光后纯命令 → None。
    #[test]
    fn clean_drops_command_echo() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"user_message","message":"<command-name>/clear</command-name>\n<command-message>clear</command-message>\n<command-args></command-args>"}}"#);
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:`<task-notification>` 与 `<local-command-stdout>` 被剥除。
    #[test]
    fn clean_drops_notification_and_stdout() {
        let ev = v(r#"{"type":"event_msg","payload":{"type":"user_message","message":"<task-notification><task-id>x</task-id></task-notification>实际输入<local-command-stdout>噪声</local-command-stdout>"}}"#);
        let (_, text, _) = extract_line(&ev).expect("应有内容");
        assert_eq!(text, "实际输入");
    }

    /// clean_message:MCP 风格工具名(`mcp__a__b`)正常提取,正文与工具分离。
    #[test]
    fn clean_extracts_mcp_tool_name() {
        let (text, tools) = clean_message("x [external_agent_tool_call: mcp__codegraph__explore]\nin\n[/external_agent_tool_call] y");
        assert_eq!(tools, vec!["mcp__codegraph__explore"]);
        assert!(text.starts_with('x') && text.ends_with('y'));
        assert!(!text.contains("codegraph"));
    }

    /// clean_message:未闭合的开标签当普通文本保留(容错,不吞后续)。
    #[test]
    fn clean_unclosed_tag_kept_literal() {
        let (text, tools) = clean_message("hi [external_agent_tool_result] no close here");
        assert_eq!(text, "hi [external_agent_tool_result] no close here");
        assert!(tools.is_empty());
    }

    /// project 名:cwd basename;无 cwd 回退 session_id 前 8 位。
    #[test]
    fn project_name_fallbacks() {
        assert_eq!(project_name(Some("D:\\hand\\yqnf\\yqnf-contract"), "s1"), "yqnf-contract");
        assert_eq!(project_name(None, "019fb603-abcd"), "019fb603");
        assert_eq!(project_name(Some("D:\\"), "019fb603"), "019fb603");
    }

    /// truncate:与其它采集器一致。
    #[test]
    fn truncate_works() {
        assert_eq!(truncate("abc", 5), "abc");
        let long = "a".repeat(10);
        assert_eq!(truncate(&long, 3), "aaa…");
    }

    /// 跨天切片:同 session 两条 event 分属不同日,只留目标日。
    /// (parse_session 是集成函数;此处用真实 rollout 行结构构造最小用例。)
    #[test]
    fn parse_session_cross_day_slice() {
        // 构造一个临时 rollout 文件:07-31 的 user_message + 08-01 的 agent_message。
        let dir = std::env::temp_dir().join("codex_collector_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("rollout-2026-07-31T10-00-00-abc.jsonl");
        let body = format!(
            concat!(
                r#"{{"timestamp":"{t0}","type":"session_meta","payload":{{"cwd":"D:\\proj","session_id":"abc"}}}}"#, "\n",
                r#"{{"timestamp":"{t0}","type":"event_msg","payload":{{"type":"user_message","message":"七月的问题"}}}}"#, "\n",
                r#"{{"timestamp":"{t1}","type":"event_msg","payload":{{"type":"agent_message","message":"八月的回复"}}}}"#, "\n"
            ),
            t0 = "2026-07-31T02:00:00Z",
            t1 = "2026-08-01T01:00:00Z"
        );
        fs::write(&path, body).unwrap();

        let d0731 = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        let (opt, skipped) = parse_session(&path, "Codex", d0731);
        let d = opt.expect("应有 digest");
        assert_eq!(skipped, 0);
        assert_eq!(d.lines.len(), 1);
        assert!(matches!(d.lines[0].role, Role::User));
        assert_eq!(d.lines[0].text, "七月的问题");
        assert_eq!(d.cwd.as_deref(), Some("D:\\proj"));
        assert_eq!(d.project, "proj");
        assert!(d.started_at.starts_with("2026-07-31"));

        // 目标日 08-01 → 只留 agent_message。
        let d0801 = NaiveDate::from_ymd_opt(2026, 8, 1).unwrap();
        let (opt, _) = parse_session(&path, "Codex", d0801);
        let d = opt.expect("应有 digest");
        assert_eq!(d.lines.len(), 1);
        assert_eq!(d.lines[0].text, "八月的回复");

        // 目标日无行 → None。
        let d0701 = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let (opt, _) = parse_session(&path, "Codex", d0701);
        assert!(opt.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 端到端:对真实 `~/.codex/sessions` 采集 2026-07-31(已知有"hi"会话)。
    /// 需本机装过 Codex,默认 ignored,手动跑:
    /// `cargo test --manifest-path src-tauri/Cargo.toml collect_real_codex_sample_day -- --ignored --nocapture`
    #[test]
    #[ignore = "需要本地 Codex sessions,仅手动验证"]
    fn collect_real_codex_sample_day() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        let (digests, skipped) = CodexCollector
            .collect(date, &PathFilter::default())
            .expect("采集不应报错");
        println!("== Codex 采集 2026-07-31: sessions={}, skipped={} ==", digests.len(), skipped);
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
        assert!(!digests.is_empty(), "2026-07-31 应至少采集到一个 Codex 会话");
    }
}

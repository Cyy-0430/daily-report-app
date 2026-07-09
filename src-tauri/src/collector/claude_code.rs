//! Claude Code 对话采集器。
//!
//! 数据源:`~/.claude/projects/<编码项目路径>/*.jsonl`,每行一个事件 JSON。
//!
//! - **时间过滤(硬契约)**:按每行 `timestamp`(UTC, RFC3339)转本地时区后比
//!   date,绝不按文件修改时间——session 跨天累积。
//! - **字段过滤(策略①)**:保留 user 文本 + assistant 文本 + tool_use 的
//!   name+关键参数;丢弃 tool_result 全文与 thinking。

use super::{session_tokens, Collector, ConversationLine, Role, SessionDigest};
use chrono::{DateTime, Local, NaiveDate};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct ClaudeCodeCollector;

impl Collector for ClaudeCodeCollector {
    fn id(&self) -> &'static str {
        "claude-code"
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn collect(&self, date: NaiveDate) -> Result<(Vec<SessionDigest>, usize), String> {
        let base = home_projects_dir()?;
        let mut digests = Vec::new();
        let mut skipped = 0usize;

        let proj_dirs = match fs::read_dir(&base) {
            Ok(it) => it,
            Err(e) => return Err(format!("读取 Claude 目录失败:{}: {e}", base.display())),
        };
        for entry in proj_dirs.flatten() {
            let proj_path = entry.path();
            if !proj_path.is_dir() {
                continue;
            }
            let project_name = proj_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let jsonl_files = match fs::read_dir(&proj_path) {
                Ok(it) => it,
                Err(_) => continue,
            };
            for f in jsonl_files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let (digest_opt, sk) = parse_session(&p, &project_name, self.display_name(), date);
                skipped += sk;
                if let Some(d) = digest_opt {
                    if !d.lines.is_empty() {
                        digests.push(d);
                    }
                }
            }
        }

        digests.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        Ok((digests, skipped))
    }
}

/// `~/.claude/projects`
fn home_projects_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())?;
    Ok(home.join(".claude").join("projects"))
}

/// 解析单个 jsonl 文件为 session(仅保留目标日期的行)。
/// 返回 (Option<SessionDigest>, 跳过行数)。
fn parse_session(
    path: &Path,
    project_name: &str,
    tool_name: &str,
    date: NaiveDate,
) -> (Option<SessionDigest>, usize) {
    let session_id = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return (None, 0),
    };

    let mut lines: Vec<ConversationLine> = Vec::new();
    let mut cwd: Option<String> = None;
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

        // 时间过滤(硬契约):UTC → 本地时区 → 比 date。
        let Some(ts_str) = ev["timestamp"].as_str() else {
            skipped += 1;
            continue;
        };
        let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) else {
            skipped += 1;
            continue;
        };
        let local = dt.with_timezone(&Local);
        if local.date_naive() != date {
            continue; // 非目标日期:正常过滤,不计入 skipped
        }
        let ts_disp = local.format("%H:%M").to_string();

        if cwd.is_none() {
            if let Some(c) = ev["cwd"].as_str() {
                cwd = Some(c.to_string());
            }
        }
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
    let digest = SessionDigest {
        tool: tool_name.to_string(),
        project: project_name.to_string(),
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

/// 策略①字段过滤:从一条事件提取 (角色, 文本, 工具摘要)。无有效内容返回 None。
/// 跳过 tool_result 全文与 thinking;tool_use 仅保留 name + 关键参数。
fn extract_line(ev: &Value) -> Option<(Role, String, Vec<String>)> {
    let ty = ev["type"].as_str()?;
    let content = &ev["message"]["content"];
    match ty {
        "user" => {
            if let Some(s) = content.as_str() {
                let s = s.trim();
                if s.is_empty() {
                    return None;
                }
                return Some((Role::User, s.to_string(), Vec::new()));
            }
            if let Some(arr) = content.as_array() {
                let mut texts = Vec::new();
                for b in arr {
                    if b["type"].as_str() == Some("text") {
                        if let Some(t) = b["text"].as_str() {
                            let t = t.trim();
                            if !t.is_empty() {
                                texts.push(t.to_string());
                            }
                        }
                    }
                }
                if texts.is_empty() {
                    return None;
                }
                return Some((Role::User, texts.join("\n"), Vec::new()));
            }
            None
        }
        "assistant" => {
            let arr = content.as_array()?;
            let mut texts = Vec::new();
            let mut tools = Vec::new();
            for b in arr {
                match b["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = b["text"].as_str() {
                            let t = t.trim();
                            if !t.is_empty() {
                                texts.push(t.to_string());
                            }
                        }
                    }
                    Some("tool_use") => {
                        let name = b["name"].as_str().unwrap_or("tool");
                        let inp = &b["input"];
                        let key = inp["file_path"]
                            .as_str()
                            .or_else(|| inp["path"].as_str())
                            .or_else(|| inp["command"].as_str())
                            .or_else(|| inp["pattern"].as_str())
                            .or_else(|| inp["url"].as_str())
                            .unwrap_or("");
                        let key = truncate(key, 80);
                        tools.push(if key.is_empty() {
                            name.to_string()
                        } else {
                            format!("{name}: {key}")
                        });
                    }
                    _ => {}
                }
            }
            if texts.is_empty() && tools.is_empty() {
                return None;
            }
            Some((Role::Assistant, texts.join("\n"), tools))
        }
        _ => None,
    }
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

    /// 纯函数:截断。
    #[test]
    fn truncate_works() {
        assert_eq!(truncate("abc", 5), "abc");
        let long = "a".repeat(10);
        assert_eq!(truncate(&long, 3), "aaa…");
    }

    /// 策略①:user 文本保留。
    #[test]
    fn extract_user_text() {
        let ev = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": "帮我实现登录" }
        });
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::User));
        assert_eq!(text, "帮我实现登录");
        assert!(tools.is_empty());
    }

    /// 策略①:纯 tool_result 被丢弃(无有效内容)。
    #[test]
    fn extract_skips_tool_result() {
        let ev = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": [
                { "type": "tool_result", "content": "巨长的文件全文……" }
            ] }
        });
        assert!(extract_line(&ev).is_none());
    }

    /// 策略①:assistant 保留 text,丢弃 thinking,tool_use 仅留 name+参数。
    #[test]
    fn extract_assistant_tools() {
        let ev = serde_json::json!({
            "type": "assistant",
            "message": { "role": "assistant", "content": [
                { "type": "thinking", "thinking": "内部推理……" },
                { "type": "text", "text": "好的，我先看一下" },
                { "type": "tool_use", "name": "Read", "input": { "file_path": "src/a.ts" } }
            ] }
        });
        let (role, text, tools) = extract_line(&ev).expect("应有内容");
        assert!(matches!(role, Role::Assistant));
        assert_eq!(text, "好的，我先看一下");
        assert_eq!(tools, vec!["Read: src/a.ts"]);
    }
}

//! 本地 AI 工具对话采集器。
//!
//! 采集与生成解耦:采集是纯本地、无 LLM、无 token 的操作,仅读取各工具
//! 存储在本机的对话记录,经字段级过滤(策略①)后渲染为一段文本,供模板
//! 变量 `{{conversations}}` 使用。新增工具只需实现 [`Collector`] trait。
//!
//! 跨层契约:jsonl 事件为 append-only 日志,解码集中在各 Collector 内部并
//! 产出类型化投影 [`ConversationLine`];过滤与渲染只消费该类型,不直接
//! cast 原始 jsonl 字段。

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod claude_code;
pub use claude_code::ClaudeCodeCollector;

/// 对话角色。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// 一条对话事件经字段级过滤后的类型化投影。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationLine {
    /// 本地时区时间(展示用,如 21:23)。
    pub ts: String,
    pub role: Role,
    /// user / assistant 的可见文本。
    pub text: String,
    /// tool_use 摘要,如 ["Read: src/auth.ts"]。
    pub tools: Vec<String>,
}

/// 单个会话摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDigest {
    /// 工具显示名,如 "Claude Code"。
    pub tool: String,
    /// 编码后的项目目录名。
    pub project: String,
    pub cwd: Option<String>,
    pub session_id: String,
    /// 本地时区起止时间。
    pub started_at: String,
    pub ended_at: String,
    pub line_count: usize,
    pub est_tokens: usize,
    pub lines: Vec<ConversationLine>,
}

/// 采集结果。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectResult {
    pub sessions: Vec<SessionDigest>,
    /// 渲染后的 `{{conversations}}` 文本。
    pub rendered_text: String,
    pub est_tokens: usize,
    /// 解析失败 / 跳过的行数(健康度参考)。
    pub skipped_lines: usize,
}

/// 路径过滤规则(已规范化的路径)。
///
/// - `includes`(白名单)非空:仅采集落在任一路径下(含自身、含子目录)的会话;
/// - `excludes`(黑名单):其下会话一律剔除;**排除优先于仅采集**。
/// 两者均为空时不过滤(默认行为)。
#[derive(Debug, Clone, Default)]
pub struct PathFilter {
    pub includes: Vec<PathBuf>,
    pub excludes: Vec<PathBuf>,
}

/// 命令层接收的路径过滤参数(原始字符串,尚未规范化)。
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PathFilterParam {
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

impl PathFilterParam {
    /// 规范化为 [`PathFilter`]:去空白/空串,统一分隔符并小写。
    pub fn normalize(&self) -> PathFilter {
        let to_paths = |xs: &[String]| {
            xs.iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(claude_code::norm)
                .collect::<Vec<_>>()
        };
        PathFilter {
            includes: to_paths(&self.include_paths),
            excludes: to_paths(&self.exclude_paths),
        }
    }
}

/// 采集器抽象。新增工具实现本 trait,并在 [`collect_conversations`] 路由中注册。
pub trait Collector: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// 采集指定本地日期的对话,并按 `filter` 做真实 cwd 路径过滤。
    /// 返回 (会话摘要, 跳过行数)。
    fn collect(
        &self,
        date: NaiveDate,
        filter: &PathFilter,
    ) -> Result<(Vec<SessionDigest>, usize), String>;
}

/// token 估算(经验值:中文 ~1.2 tok/字,ASCII ~0.25 tok/char)。仅作预览参考,不用于计费。
pub fn estimate_tokens(s: &str) -> usize {
    let mut non_ascii = 0usize;
    for c in s.chars() {
        if (c as u32) > 127 {
            non_ascii += 1;
        }
    }
    let ascii = s.chars().count() - non_ascii;
    (non_ascii as f64 * 1.2 + ascii as f64 * 0.25) as usize
}

/// 估算单个 session 所有行的 token。
pub fn session_tokens(lines: &[ConversationLine]) -> usize {
    let mut buf = String::new();
    for l in lines {
        buf.push_str(&l.text);
        for t in &l.tools {
            buf.push_str(t);
        }
    }
    estimate_tokens(&buf)
}

/// 把多个 session 渲染为 `{{conversations}}` 文本,返回 (文本, token)。
pub fn render(sessions: &[SessionDigest]) -> (String, usize) {
    let mut out = String::new();
    for s in sessions {
        if s.lines.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "### {} · {}\n> 项目 {} | {} ~ {} | {} 条\n\n",
            s.tool,
            s.project,
            s.cwd.as_deref().unwrap_or("-"),
            s.started_at,
            s.ended_at,
            s.line_count
        ));
        for ln in &s.lines {
            out.push('[');
            out.push_str(&ln.ts);
            out.push_str("] ");
            out.push_str(match ln.role {
                Role::User => "用户",
                Role::Assistant => "助手",
            });
            out.push_str(": ");
            out.push_str(&ln.text);
            if !ln.tools.is_empty() {
                out.push_str("\n  调用工具: ");
                out.push_str(&ln.tools.join("  |  "));
            }
            out.push('\n');
        }
        out.push('\n');
    }
    let tokens = estimate_tokens(&out);
    (out, tokens)
}

/// 解析日期参数:"YYYY-MM-DD";空串或非法 → 今天(本地时区)。
fn parse_target_date(date: &str) -> NaiveDate {
    match NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => Local::now().date_naive(),
    }
}

/// 所有已注册的采集器(新增工具只需在此登记一处)。
fn all_collectors() -> Vec<Box<dyn Collector>> {
    vec![Box::new(ClaudeCodeCollector)]
}

/// 采集(同步阻塞 IO),由 command 在 spawn_blocking 中调用。
fn collect_blocking(
    date: &str,
    tools: &[String],
    filter: &PathFilter,
) -> Result<CollectResult, String> {
    let target = parse_target_date(date);
    let mut result = CollectResult::default();
    for c in all_collectors() {
        if !tools.iter().any(|t| t == c.id()) {
            continue; // 未勾选的工具跳过
        }
        let (sessions, skipped) = c.collect(target, filter)?;
        result.skipped_lines += skipped;
        result.sessions.extend(sessions);
    }
    // 跨工具按时间统一排序。
    result.sessions.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    let (text, tokens) = render(&result.sessions);
    result.rendered_text = text;
    result.est_tokens = tokens;
    Ok(result)
}

/// 采集指定日期、指定工具的本地对话记录。
///
/// - `date`:本地时区的某一天,格式 "YYYY-MM-DD";空串表示今天。
/// - `tools`:工具 id 列表,MVP 仅支持 "claude-code"。
/// - `filter`:路径过滤(include/exclude,基于真实 cwd);传空数组等价于不过滤。
#[tauri::command]
pub async fn collect_conversations(
    date: String,
    tools: Vec<String>,
    filter: PathFilterParam,
) -> Result<CollectResult, String> {
    let filter = filter.normalize();
    let date = date.clone();
    let tools = tools.clone();
    tokio::task::spawn_blocking(move || collect_blocking(&date, &tools, &filter))
        .await
        .map_err(|e| format!("采集任务异常: {e}"))?
}

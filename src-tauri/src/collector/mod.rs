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
use std::collections::HashMap;
use std::path::PathBuf;

pub mod claude_code;
pub use claude_code::ClaudeCodeCollector;

pub mod zcode;
pub use zcode::ZCodeCollector;

pub mod codex;
pub use codex::CodexCollector;

pub mod opencode;
pub use opencode::OpencodeCollector;

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
    /// 该工具数据源的默认路径(已展开 `~`)。`None` 表示无法定位(如取不到主目录)。
    /// 仅供「展示默认值」与「无覆盖时回退」使用,错误上抛由 [`Collector::collect`] 决定。
    fn default_path(&self) -> Option<PathBuf>;
    /// 采集指定本地日期的对话,并按 `filter` 做真实 cwd 路径过滤。
    /// `custom_path` 非空(去空白后)→ 用它(展开 `~`);否则用 [`Collector::default_path`]。
    /// 返回 (会话摘要, 跳过行数)。
    fn collect(
        &self,
        date: NaiveDate,
        filter: &PathFilter,
        custom_path: Option<&str>,
    ) -> Result<(Vec<SessionDigest>, usize), String>;
}

/// 把用户输入的路径串解析为实际路径:展开 `~` / `~/x` / `~\x` 为真实主目录;
/// 空串(或纯空白)→ `None`(表示「无覆盖,用默认」);其余原样返回。
///
/// 各采集器统一按 `custom_path.and_then(expand_home).or_else(default_path)` 解析,
/// 保证「空覆盖 = 用默认」这一跨层语义在 Rust/TS 两侧一致。
pub(super) fn expand_home(p: &str) -> Option<PathBuf> {
    let p = p.trim();
    if p.is_empty() {
        return None;
    }
    match p {
        "~" => dirs::home_dir(),
        s if s.starts_with("~/") => dirs::home_dir().map(|h| h.join(&s[2..])),
        s if s.starts_with("~\\") => dirs::home_dir().map(|h| h.join(&s[2..])),
        s => Some(PathBuf::from(s)),
    }
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
pub(crate) fn parse_target_date(date: &str) -> NaiveDate {
    match NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => Local::now().date_naive(),
    }
}

/// 所有已注册的采集器(新增工具只需在此登记一处)。
fn all_collectors() -> Vec<Box<dyn Collector>> {
    vec![
        Box::new(ClaudeCodeCollector),
        Box::new(ZCodeCollector),
        Box::new(CodexCollector),
        Box::new(OpencodeCollector),
    ]
}

/// 采集(同步阻塞 IO),由 command 在 spawn_blocking 中调用。
///
/// 单日切片:collector trait 只认单个 [`NaiveDate`]。区间采集(周报)在命令层
/// 逐日循环调用本函数(见 [`collect_conversations_range`]),无需泛化 trait。
fn collect_blocking(
    date: NaiveDate,
    tools: &[String],
    filter: &PathFilter,
    tool_paths: &HashMap<String, String>,
) -> Result<CollectResult, String> {
    let mut result = CollectResult::default();
    for c in all_collectors() {
        if !tools.iter().any(|t| t == c.id()) {
            continue; // 未勾选的工具跳过
        }
        // 该工具的自定义数据源路径:取键、去空白、非空才传 Some(~ 由采集器展开)。
        let custom = tool_paths
            .get(c.id())
            .map(|s| s.as_str())
            .filter(|s| !s.trim().is_empty());
        let (sessions, skipped) = c.collect(date, filter, custom)?;
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

/// 逐日采集区间 `[start, end]`(含首尾,自动处理倒序),返回每日的 [`CollectResult`]
/// (含空日,调用方按需过滤)。周报采集命令与 `generate_weekly_report` 共用此函数。
pub(crate) fn collect_range_days(
    start: NaiveDate,
    end: NaiveDate,
    tools: &[String],
    filter: &PathFilter,
    tool_paths: &HashMap<String, String>,
) -> Result<Vec<(NaiveDate, CollectResult)>, String> {
    let (start, end) = if end < start { (end, start) } else { (start, end) };
    let mut out = Vec::new();
    let mut d = start;
    loop {
        let res = collect_blocking(d, tools, filter, tool_paths)?;
        out.push((d, res));
        match d.succ_opt() {
            Some(next) if next <= end => d = next,
            _ => break,
        }
    }
    Ok(out)
}

/// 采集指定日期、指定工具的本地对话记录。
///
/// - `date`:本地时区的某一天,格式 "YYYY-MM-DD";空串表示今天。
/// - `tools`:工具 id 列表,支持 "claude-code"、"zcode"、"codex" 与 "opencode"。
/// - `filter`:路径过滤(include/exclude,基于真实 cwd);传空数组等价于不过滤。
/// - `tool_paths`:各工具的自定义数据源路径(覆盖默认);键缺失或空串 = 用默认。
#[tauri::command]
pub async fn collect_conversations(
    date: String,
    tools: Vec<String>,
    filter: PathFilterParam,
    tool_paths: HashMap<String, String>,
) -> Result<CollectResult, String> {
    let filter = filter.normalize();
    let target = parse_target_date(&date);
    let tools = tools.clone();
    tokio::task::spawn_blocking(move || collect_blocking(target, &tools, &filter, &tool_paths))
        .await
        .map_err(|e| format!("采集任务异常: {e}"))?
}

/// 区间采集的单日结果(周报 map 的一个批次)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayCollect {
    /// "YYYY-MM-DD"。
    pub date: String,
    pub sessions: Vec<SessionDigest>,
    /// 当日渲染后的对话文本(喂给 map 摘要)。
    pub rendered_text: String,
    pub est_tokens: usize,
}

/// 区间采集结果:区间内逐日明细 + 总 token(供前端预览/预算,不耗 LLM)。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeCollectResult {
    /// 按日期升序;无对话的日期也保留(est_tokens=0),便于前端逐日展示。
    pub days: Vec<DayCollect>,
    pub total_tokens: usize,
    pub skipped_lines: usize,
}

/// 采集区间内(含首尾)逐日的对话记录。逐日以单日切片采集(复用 [`collect_blocking`]),
/// 每日一个 [`DayCollect`](周报 map 的一个批次)。仅本地 IO,无 LLM。
///
/// - `start`/`end`:本地日期 "YYYY-MM-DD",空/非法 → 今天;`end < start` 时自动交换。
/// - `tools`/`filter`/`tool_paths` 语义同 [`collect_conversations`]。
#[tauri::command]
pub async fn collect_conversations_range(
    start: String,
    end: String,
    tools: Vec<String>,
    filter: PathFilterParam,
    tool_paths: HashMap<String, String>,
) -> Result<RangeCollectResult, String> {
    let filter = filter.normalize();
    let start_d = parse_target_date(&start);
    let end_d = parse_target_date(&end);
    let tools = tools.clone();
    tokio::task::spawn_blocking(move || {
        let pairs = collect_range_days(start_d, end_d, &tools, &filter, &tool_paths)?;
        let mut days = Vec::with_capacity(pairs.len());
        let mut total_tokens = 0usize;
        let mut skipped_lines = 0usize;
        for (d, res) in pairs {
            total_tokens += res.est_tokens;
            skipped_lines += res.skipped_lines;
            days.push(DayCollect {
                date: d.format("%Y-%m-%d").to_string(),
                sessions: res.sessions,
                rendered_text: res.rendered_text,
                est_tokens: res.est_tokens,
            });
        }
        Ok(RangeCollectResult {
            days,
            total_tokens,
            skipped_lines,
        })
    })
    .await
    .map_err(|e| format!("区间采集任务异常: {e}"))?
}

/// 返回各采集工具数据源的**默认路径**(已展开 `~`),供设置页展示与「恢复默认」使用。
/// 键 = 工具 id,值 = 真实路径串;无法定位主目录的工具不出现在结果中。
#[tauri::command]
pub fn default_collect_paths() -> HashMap<String, String> {
    all_collectors()
        .into_iter()
        .filter_map(|c| {
            c.default_path()
                .map(|p| (c.id().to_string(), p.to_string_lossy().into_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{collect_range_days, expand_home, PathFilter};
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// `~` / `~/x` / `~\x` 展开为真实主目录;空串与纯空白 → None;绝对/无前缀原样返回。
    #[test]
    fn expand_home_tilde_and_plain() {
        let home = dirs::home_dir().expect("测试环境应能定位主目录");

        // `~` 单独 → 主目录本身。
        assert_eq!(expand_home("~"), Some(home.clone()));
        // `~/x` → 主目录下 x(正斜杠)。
        assert_eq!(expand_home("~/x"), Some(home.join("x")));
        // `~\x` → 主目录下 x(反斜杠,Windows 写法)。
        assert_eq!(expand_home("~\\x"), Some(home.join("x")));

        // 空串 / 纯空白 → None(= 无覆盖,用默认)。
        assert_eq!(expand_home(""), None);
        assert_eq!(expand_home("   "), None);

        // 绝对路径 / 无 `~` 前缀 → 原样返回(去首尾空白)。
        assert_eq!(expand_home("D:/work/app"), Some(PathBuf::from("D:/work/app")));
        assert_eq!(expand_home("  D:\\work  "), Some(PathBuf::from("D:\\work")));
    }

    /// 区间循环:含首尾、升序、end<start 自动交换、单日特例。用空工具列表(不触盘)。
    #[test]
    fn collect_range_days_inclusive_swap_and_single() {
        let no_tools: Vec<String> = vec![];
        let filter = PathFilter::default();
        let paths = HashMap::new();
        let d1 = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2026, 8, 4).unwrap();
        let d3 = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();

        // 正常区间:含首尾,升序 3 天。
        let days = collect_range_days(d1, d3, &no_tools, &filter, &paths).unwrap();
        let dates: Vec<_> = days.iter().map(|(d, _)| *d).collect();
        assert_eq!(dates, vec![d1, d2, d3]);

        // 倒序 end<start:自动交换,仍升序。
        let days = collect_range_days(d3, d1, &no_tools, &filter, &paths).unwrap();
        let dates: Vec<_> = days.iter().map(|(d, _)| *d).collect();
        assert_eq!(dates, vec![d1, d2, d3]);

        // 单日区间 = 1 天(周报「单日周报」特例)。
        let days = collect_range_days(d2, d2, &no_tools, &filter, &paths).unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0].0, d2);
    }
}

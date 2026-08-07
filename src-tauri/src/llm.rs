use crate::collector::{collect_range_days, parse_target_date, FMT_DATE, PathFilterParam};
use crate::config::{load_config, ApiConfig, HistoryItem};
use crate::db::{insert_history, DbState};
use chrono::{Datelike, NaiveDate};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

// ===========================================================================
// LLM 调用相关常量(端点 / SSE / 模板变量 / 消息 / stage / 标题)
// ===========================================================================
// 注:含 `{}` 占位的 `format!` 模板(如 "请求失败：{e}")因 Rust 宏要求字面量,无法提
// 为 const,保留就近字面量;此处仅收集纯字面量。

/// OpenAI 兼容端点路径片段(build_endpoint 拼接)。
const CHAT_COMPLETIONS: &str = "/chat/completions";
const V1: &str = "/v1";

/// SSE 流标记。
const SSE_DATA: &str = "data:";
const SSE_DONE: &str = "[DONE]";

/// 模板变量占位符(render_* 注入)。
const TPL_DATE: &str = "{{date}}";
const TPL_INPUT: &str = "{{input}}";
const TPL_CONV: &str = "{{conversations}}";
const TPL_DATE_RANGE: &str = "{{date_range}}";
const TPL_DAY_SUMMARIES: &str = "{{day_summaries}}";

/// API/连接/响应相关提示文案。
const MSG_API_INCOMPLETE: &str = "请先在设置中填写完整的 API 配置（BaseURL / Key / 模型）";
const MSG_API_INCOMPLETE_SHORT: &str = "请填写完整的 API 配置";
const MSG_RESPONSE_MISSING_CONTENT: &str = "响应缺少 choices[0].message.content";
const MSG_CONNECT_OK: &str = "连接成功";
const MSG_NO_WEEKLY_MATERIAL: &str = "区间内无有效对话，且未填写补充要点，无法生成周报";

/// 周报 map/reduce 进度阶段。
const STAGE_MAP: &str = "map";
const STAGE_REDUCE: &str = "reduce";

/// 标题后缀与展示分隔符。
const TITLE_DAILY_SUFFIX: &str = "日报";
const TITLE_WEEKLY_SUFFIX: &str = "周报";
const DATE_RANGE_SEP: &str = "~";
const MISSING_JOIN_SEP: &str = "、";
const EMPTY_SUMMARIES: &str = "（无）";

/// 流式事件，通过 Tauri Channel 推送到前端。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StreamChunk {
    Delta { text: String },
    Done,
    Error { message: String },
    /// 周报 map/reduce 进度(前端展示步骤文案 + 进度条)。
    Progress {
        stage: String,
        current: usize,
        total: usize,
        message: String,
    },
}

/// 周/日报重试次数(额外重试数,总尝试 = retries+1)。
const RETRIES: u32 = 3;

pub fn render_template(
    template: &str,
    input: &str,
    date_md: &str,
    conversations: &str,
) -> String {
    template
        .replace(TPL_DATE, date_md)
        .replace(TPL_INPUT, input)
        .replace(TPL_CONV, conversations)
}

/// 渲染周报 map(每日摘要)提示词。
fn render_weekly_map(template: &str, date: &str, conversations: &str) -> String {
    template
        .replace(TPL_DATE, date)
        .replace(TPL_CONV, conversations)
}

/// 渲染周报 reduce(整周汇总)提示词。
fn render_weekly_reduce(template: &str, date_range: &str, input: &str, day_summaries: &str) -> String {
    template
        .replace(TPL_DATE_RANGE, date_range)
        .replace(TPL_INPUT, input)
        .replace(TPL_DAY_SUMMARIES, day_summaries)
}

/// 区间 → "M.d–M.d" 展示串(标题与 {{date_range}} 共用)。
fn format_range_md(start: NaiveDate, end: NaiveDate) -> String {
    format!(
        "{}.{}–{}.{}",
        start.month(),
        start.day(),
        end.month(),
        end.day()
    )
}

/// 单日 → "M.d"。
fn date_md(d: NaiveDate) -> String {
    format!("{}.{}", d.month(), d.day())
}

/// API 配置是否完整。
fn api_incomplete(api: &ApiConfig) -> bool {
    api.base_url.is_empty() || api.api_key.is_empty() || api.model.is_empty()
}

/// 周报 map(每日摘要)默认提示词(配置为空时兜底)。
const WEEKLY_MAP_PROMPT: &str = "你是工作摘要助手。请把下面「某一天」的 AI 工具对话记录提炼成一份简洁的当日工作摘要，供后续汇总周报使用。

日期：{{date}}

当日 AI 工具对话记录：
{{conversations}}

要求：
- 用条目列出当天实际做了哪些事（实现/修复/调研/重构等），合并琐碎细节，突出进展与成果；
- 简要标注遇到的关键问题或阻塞（没有则不写）；
- 控制在 150 字以内，不要复述对话原文，不要寒暄或解释，只输出摘要本身。";

/// 周报 reduce(整周汇总)默认提示词(配置为空时兜底)。
const WEEKLY_REDUCE_PROMPT: &str = "你是周报整理助手。请把下面本周各天的「当日工作摘要」跨天归纳成一份整周周报。

本周区间：{{date_range}}

本周补充要点（用户手写，可能为空）：
{{input}}

本周各日工作摘要：
{{day_summaries}}

要求格式：

{{date_range}}周报：

## 本周工作事项
1、...
2、...

## 遇到问题
...

## 总结
...

要求：
- 「工作事项」跨天合并同类项，按主题/项目组织，用「数字、」编号，每组换行；提炼成果与进展，不要堆砌流水账；
- 「遇到问题」如实归纳本周的主要阻碍，没有就写「无」；
- 「总结」用一两句话概括本周工作的核心思路或价值，专业简练；
- 若「补充要点」非空，将其纳入对应主题；
- 严格只输出周报正文，开头不加问候/确认/解释，结尾不加总结性询问。";

/// 规范化 OpenAI 兼容接口的请求地址。
fn build_endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with(CHAT_COMPLETIONS) {
        base.to_string()
    } else if base.ends_with(V1) {
        format!("{base}{CHAT_COMPLETIONS}")
    } else {
        format!("{base}{V1}{CHAT_COMPLETIONS}")
    }
}

/// 流式调用的一次尝试:逐字通过 Channel 推送 Delta,返回完整文本。
/// 首个 delta 推送后置 `emitted=true`(供重试编排判断「已部分输出」)。
/// 不发 Done/Error——由调用方在成功/失败后发送,以便重试编排控制。
async fn stream_chat_once(
    api: &ApiConfig,
    prompt: &str,
    on_event: &Channel<StreamChunk>,
    emitted: &AtomicBool,
) -> Result<String, String> {
    let endpoint = build_endpoint(&api.base_url);
    let body = serde_json::json!({
        "model": api.model,
        "stream": true,
        "messages": [{ "role": "user", "content": prompt }]
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&endpoint)
        .bearer_auth(&api.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {status}：{text}"));
    }
    let mut full = String::new();
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buf.find('\n') {
            let line: String = buf[..pos].trim().to_string();
            buf.drain(..=pos);
            if line.is_empty() || !line.starts_with(SSE_DATA) {
                continue;
            }
            let data = line[SSE_DATA.len()..].trim();
            if data == SSE_DONE {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                    full.push_str(delta);
                    emitted.store(true, Ordering::SeqCst);
                    let _ = on_event.send(StreamChunk::Delta {
                        text: delta.to_string(),
                    });
                }
            }
        }
    }
    Ok(full)
}

/// 非流式单次调用(周报 map 用):返回 choices[0].message.content。
async fn complete_once(api: &ApiConfig, prompt: &str) -> Result<String, String> {
    let endpoint = build_endpoint(&api.base_url);
    let body = serde_json::json!({
        "model": api.model,
        "stream": false,
        "messages": [{ "role": "user", "content": prompt }]
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&endpoint)
        .bearer_auth(&api.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("API 返回错误 {status}：{text}"));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| MSG_RESPONSE_MISSING_CONTENT.to_string())
}

/// 非流式调用 + 指数退避重试(backoff 1s,2s,4s)。用于周报 map。
async fn complete_with_retry(
    api: &ApiConfig,
    prompt: &str,
    retries: u32,
) -> Result<String, String> {
    let mut delay = Duration::from_secs(1);
    let mut last_err = String::new();
    for attempt in 0..=retries {
        if attempt > 0 {
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2);
        }
        match complete_once(api, prompt).await {
            Ok(v) => return Ok(v),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// 流式调用 + 指数退避重试(backoff 1s,2s,4s)。仅当**尚未发出任何 delta**时才重试,
/// 避免已部分输出后再重试导致前端重复内容。成功后由调用方发送 Done。
async fn stream_chat_retry(
    api: &ApiConfig,
    prompt: &str,
    on_event: &Channel<StreamChunk>,
    retries: u32,
) -> Result<String, String> {
    let emitted = AtomicBool::new(false);
    let mut delay = Duration::from_secs(1);
    let mut last_err = String::new();
    for attempt in 0..=retries {
        if attempt > 0 {
            tokio::time::sleep(delay).await;
            delay = delay.saturating_mul(2);
        }
        match stream_chat_once(api, prompt, on_event, &emitted).await {
            Ok(t) => return Ok(t),
            Err(e) => {
                last_err = e;
                if emitted.load(Ordering::SeqCst) {
                    break; // 已部分输出:重试会造成前端重复 → 终止
                }
            }
        }
    }
    Err(last_err)
}

/// 调用 LLM 流式生成日报，逐字通过 Channel 推送，返回完整文本。
pub async fn generate_stream(
    api: &ApiConfig,
    template: &str,
    input: &str,
    conversations: &str,
    on_event: &Channel<StreamChunk>,
) -> Result<String, String> {
    if api_incomplete(api) {
        let msg = MSG_API_INCOMPLETE;
        let _ = on_event.send(StreamChunk::Error {
            message: msg.into(),
        });
        return Err(msg.into());
    }

    let now = chrono::Local::now();
    let date_md = format!("{}.{}", now.month(), now.day());
    let prompt = render_template(template, input, &date_md, conversations);

    let emitted = AtomicBool::new(false);
    match stream_chat_once(api, &prompt, on_event, &emitted).await {
        Ok(full) => {
            let _ = on_event.send(StreamChunk::Done);
            Ok(full)
        }
        Err(e) => {
            let _ = on_event.send(StreamChunk::Error {
                message: e.clone(),
            });
            Err(e)
        }
    }
}

/// 测试 API 连通性（非流式，小请求）。
#[tauri::command]
pub async fn test_connection(api: ApiConfig) -> Result<String, String> {
    if api_incomplete(&api) {
        return Err(MSG_API_INCOMPLETE_SHORT.into());
    }
    let endpoint = build_endpoint(&api.base_url);
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({
        "model": api.model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_tokens": 5
    });
    let resp = client
        .post(&endpoint)
        .bearer_auth(&api.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败：{e}"))?;
    if resp.status().is_success() {
        Ok(MSG_CONNECT_OK.into())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("连接失败 {status}：{text}"))
    }
}

/// 流式生成日报，完成后写入历史记录(独立 `add_history`,不再全量读写配置)。
#[tauri::command]
pub async fn generate_report(
    app: AppHandle,
    input: String,
    conversations: String,
    on_event: Channel<StreamChunk>,
) -> Result<HistoryItem, String> {
    let cfg = load_config(app.clone())?;
    let full = generate_stream(
        &cfg.api_config,
        &cfg.prompt_template,
        &input,
        &conversations,
        &on_event,
    )
    .await?;

    let now = chrono::Local::now();
    let item = HistoryItem {
        id: uuid::Uuid::new_v4().to_string(),
        date: now.format(FMT_DATE).to_string(),
        title: format!("{}.{}{TITLE_DAILY_SUFFIX}", now.month(), now.day()),
        input,
        output: full,
        created_at: now.timestamp(),
    };
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    insert_history(&conn, &item)?;
    Ok(item)
}

/// 流式生成周报(map-reduce):区间采集→逐日摘要(map,重试+失败跳过)→整周汇总(reduce,
/// 重试+失败报错),完成后写入历史。采集无 LLM;map 用非流式,reduce 用流式(逐字推送)。
#[tauri::command]
pub async fn generate_weekly_report(
    app: AppHandle,
    start: String,
    end: String,
    tools: Vec<String>,
    filter: PathFilterParam,
    tool_paths: HashMap<String, String>,
    weekly_input: String,
    on_event: Channel<StreamChunk>,
) -> Result<HistoryItem, String> {
    let cfg = load_config(app.clone())?;
    let api = &cfg.api_config;
    if api_incomplete(api) {
        let msg = MSG_API_INCOMPLETE;
        let _ = on_event.send(StreamChunk::Error {
            message: msg.into(),
        });
        return Err(msg.into());
    }

    // ① 区间采集(逐日,无 LLM)。
    let mut start_d = parse_target_date(&start);
    let mut end_d = parse_target_date(&end);
    if end_d < start_d {
        std::mem::swap(&mut start_d, &mut end_d); // 与 collect_range_days 语义一致,标题/日期用升序
    }
    let filter = filter.normalize();
    let pairs = tokio::task::spawn_blocking(move || {
        collect_range_days(start_d, end_d, &tools, &filter, &tool_paths)
    })
    .await
    .map_err(|e| format!("区间采集任务异常: {e}"))??;
    // 仅保留有对话的日期作为 map 批次。
    let days: Vec<(NaiveDate, String)> = pairs
        .into_iter()
        .filter(|(_, r)| !r.sessions.is_empty())
        .map(|(d, r)| (d, r.rendered_text))
        .collect();

    // ② map:逐日摘要(**并发** MAP_CONCURRENCY 路,每批重试3次,失败跳过并记录)。
    // 并发只加速请求往返;结果按原始日期顺序收集,reduce 输入仍保持时间升序。
    // 进度 current = 已完成批数(完成顺序乱序,进度条平滑推进)。
    const MAP_CONCURRENCY: usize = 3;
    let map_tpl = if cfg.weekly_map_template.trim().is_empty() {
        WEEKLY_MAP_PROMPT
    } else {
        cfg.weekly_map_template.as_str()
    };
    let total = days.len();
    let mut summaries: Vec<Option<String>> = vec![None; total];
    let mut missing: Vec<String> = Vec::new();
    let mut done = 0usize;
    // 先急切收集 future(自包含、owned),再 buffer_unordered 限并发——闭包不再依赖
    // 迭代项的生命周期,避免 FnOnce 泛化错误。
    let tasks: Vec<_> = days
        .iter()
        .enumerate()
        .map(|(i, (d, conv))| {
            let prompt = render_weekly_map(map_tpl, &date_md(*d), conv);
            let api_ref = api;
            let day = *d;
            async move {
                let res = complete_with_retry(api_ref, &prompt, RETRIES).await;
                (i, day, res)
            }
        })
        .collect();
    let mut stream = futures_util::stream::iter(tasks).buffer_unordered(MAP_CONCURRENCY);
    while let Some((i, d, res)) = stream.next().await {
        done += 1;
        match res {
            Ok(s) => {
                summaries[i] = Some(format!("### {}（{}）\n{}", date_md(d), d.format(FMT_DATE), s));
                let _ = on_event.send(StreamChunk::Progress {
                    stage: STAGE_MAP.into(),
                    current: done,
                    total,
                    message: format!("摘要 {} 完成", d.format("%m-%d")),
                });
            }
            Err(_) => {
                missing.push(d.format(FMT_DATE).to_string());
                let _ = on_event.send(StreamChunk::Progress {
                    stage: STAGE_MAP.into(),
                    current: done,
                    total,
                    message: format!("跳过 {}（失败）", d.format("%m-%d")),
                });
            }
        }
    }
    let summaries: Vec<String> = summaries.into_iter().flatten().collect();

    // ③ reduce 前置校验:完全无素材则报错。
    if summaries.is_empty() && weekly_input.trim().is_empty() {
        let msg = MSG_NO_WEEKLY_MATERIAL;
        let _ = on_event.send(StreamChunk::Error {
            message: msg.into(),
        });
        return Err(msg.into());
    }

    // ④ reduce:整周汇总(流式,重试3次,失败报错)。
    let _ = on_event.send(StreamChunk::Progress {
        stage: STAGE_REDUCE.into(),
        current: 1,
        total: 1,
        message: "汇总".into(),
    });
    let reduce_tpl = if cfg.weekly_reduce_template.trim().is_empty() {
        WEEKLY_REDUCE_PROMPT
    } else {
        cfg.weekly_reduce_template.as_str()
    };
    let mut day_summaries = if summaries.is_empty() {
        EMPTY_SUMMARIES.to_string()
    } else {
        summaries.join("\n\n")
    };
    if !missing.is_empty() {
        day_summaries.push_str(&format!(
            "\n\n> ⚠️ 以下日期因摘要失败已跳过：{}",
            missing.join(MISSING_JOIN_SEP)
        ));
    }
    let date_range = format_range_md(start_d, end_d);
    let reduce_prompt =
        render_weekly_reduce(reduce_tpl, &date_range, &weekly_input, &day_summaries);

    let final_text = match stream_chat_retry(api, &reduce_prompt, &on_event, RETRIES).await {
        Ok(t) => {
            let _ = on_event.send(StreamChunk::Done);
            t
        }
        Err(e) => {
            let _ = on_event.send(StreamChunk::Error {
                message: e.clone(),
            });
            return Err(e);
        }
    };

    // ⑤ 落库。
    let now = chrono::Local::now();
    let item = HistoryItem {
        id: uuid::Uuid::new_v4().to_string(),
        date: format!(
            "{}{DATE_RANGE_SEP}{}",
            start_d.format(FMT_DATE),
            end_d.format(FMT_DATE)
        ),
        title: format!("{}{TITLE_WEEKLY_SUFFIX}", date_range),
        input: weekly_input,
        output: final_text,
        created_at: now.timestamp(),
    };
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    insert_history(&conn, &item)?;
    Ok(item)
}

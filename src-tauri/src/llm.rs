use crate::config::{load_config, ApiConfig, HistoryItem};
use crate::db::{insert_history, DbState};
use chrono::Datelike;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

/// 流式事件，通过 Tauri Channel 推送到前端。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StreamChunk {
    Delta { text: String },
    Done,
    Error { message: String },
}

pub fn render_template(
    template: &str,
    input: &str,
    date_md: &str,
    conversations: &str,
) -> String {
    template
        .replace("{{date}}", date_md)
        .replace("{{input}}", input)
        .replace("{{conversations}}", conversations)
}

/// 规范化 OpenAI 兼容接口的请求地址。
fn build_endpoint(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

/// 调用 LLM 流式生成，逐字通过 Channel 推送，返回完整文本。
pub async fn generate_stream(
    api: &ApiConfig,
    template: &str,
    input: &str,
    conversations: &str,
    on_event: &Channel<StreamChunk>,
) -> Result<String, String> {
    if api.base_url.is_empty() || api.api_key.is_empty() || api.model.is_empty() {
        let msg = "请先在设置中填写完整的 API 配置（BaseURL / Key / 模型）";
        let _ = on_event.send(StreamChunk::Error {
            message: msg.into(),
        });
        return Err(msg.into());
    }

    let now = chrono::Local::now();
    let date_md = format!("{}.{}", now.month(), now.day());
    let prompt = render_template(template, input, &date_md, conversations);

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
        let msg = format!("API 返回错误 {status}：{text}");
        let _ = on_event.send(StreamChunk::Error {
            message: msg.clone(),
        });
        return Err(msg);
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
            if line.is_empty() || !line.starts_with("data:") {
                continue;
            }
            let data = line["data:".len()..].trim();
            if data == "[DONE]" {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                    full.push_str(delta);
                    let _ = on_event.send(StreamChunk::Delta {
                        text: delta.to_string(),
                    });
                }
            }
        }
    }

    let _ = on_event.send(StreamChunk::Done);
    Ok(full)
}

/// 测试 API 连通性（非流式，小请求）。
#[tauri::command]
pub async fn test_connection(api: ApiConfig) -> Result<String, String> {
    if api.base_url.is_empty() || api.api_key.is_empty() || api.model.is_empty() {
        return Err("请填写完整的 API 配置".into());
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
        Ok("连接成功".into())
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
        date: now.format("%Y-%m-%d").to_string(),
        title: format!("{}.{}日报", now.month(), now.day()),
        input,
        output: full,
        created_at: now.timestamp(),
    };
    let state = app.state::<DbState>();
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    insert_history(&conn, &item)?;
    Ok(item)
}

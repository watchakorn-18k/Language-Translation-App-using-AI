use crate::providers::ProviderError;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

/// Parses a single OpenAI-compatible SSE `data:` line and returns the delta text if present.
pub fn parse_openai_sse_line(line: &str) -> Option<String> {
    let payload = line.trim().strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let val: Value = serde_json::from_str(payload).ok()?;
    val.get("choices")?
        .as_array()?
        .first()?
        .get("delta")?
        .get("content")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Parses a single Gemini `streamGenerateContent?alt=sse` `data:` line.
pub fn parse_gemini_sse_line(line: &str) -> Option<String> {
    let payload = line.trim().strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let val: Value = serde_json::from_str(payload).ok()?;
    let parts = val
        .get("candidates")?
        .as_array()?
        .first()?
        .get("content")?
        .get("parts")?
        .as_array()?;
    let mut chunk = String::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
            chunk.push_str(text);
        }
    }
    if chunk.is_empty() { None } else { Some(chunk) }
}

/// Parses a single Anthropic Claude Messages SSE `data:` line (`content_block_delta`).
pub fn parse_claude_sse_line(line: &str) -> Option<String> {
    let payload = line.trim().strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let val: Value = serde_json::from_str(payload).ok()?;
    if val.get("type")?.as_str()? != "content_block_delta" {
        return None;
    }
    val.get("delta")?
        .get("text")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

async fn consume_sse_stream(
    resp: reqwest::Response,
    parse_line: fn(&str) -> Option<String>,
    tx: &UnboundedSender<String>,
) -> Result<String, ProviderError> {
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ProviderError::Api {
            status: status.as_u16(),
            body,
        });
    }

    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(item) = stream.next().await {
        let bytes = item?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].to_string();
            buffer.drain(..=pos);
            if let Some(delta) = parse_line(&line) {
                full.push_str(&delta);
                let _ = tx.send(delta);
            }
        }
    }

    if !buffer.trim().is_empty() {
        if let Some(delta) = parse_line(&buffer) {
            full.push_str(&delta);
            let _ = tx.send(delta);
        }
    }

    if full.is_empty() {
        Err(ProviderError::EmptyResponse)
    } else {
        Ok(full)
    }
}

#[derive(Debug, Serialize)]
struct StreamOpenAiReq<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    messages: Vec<StreamMsg<'a>>,
}

#[derive(Debug, Serialize)]
struct StreamMsg<'a> {
    role: &'static str,
    content: &'a str,
}

pub async fn stream_openai_compatible(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    tx: &UnboundedSender<String>,
) -> Result<String, ProviderError> {
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let payload = StreamOpenAiReq {
        model,
        temperature: 0.1,
        max_tokens: 2000,
        stream: true,
        messages: vec![
            StreamMsg {
                role: "system",
                content: system_prompt,
            },
            StreamMsg {
                role: "user",
                content: user_prompt,
            },
        ],
    };

    let client = reqwest::Client::new();
    let mut req = client.post(&endpoint).json(&payload);
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await?;
    consume_sse_stream(resp, parse_openai_sse_line, tx).await
}

pub async fn stream_gemini(
    api_key: &str,
    model: &str,
    full_prompt: &str,
    tx: &UnboundedSender<String>,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Gemini"));
    }
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent"
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": full_prompt }] }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .query(&[("key", api_key.trim()), ("alt", "sse")])
        .json(&body)
        .send()
        .await?;

    consume_sse_stream(resp, parse_gemini_sse_line, tx).await
}

pub async fn stream_claude(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    tx: &UnboundedSender<String>,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Claude"));
    }
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 2000,
        "stream": true,
        "system": system_prompt,
        "messages": [{ "role": "user", "content": user_prompt }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await?;

    consume_sse_stream(resp, parse_claude_sse_line, tx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_sse_deltas_and_done() {
        let line = r#"data: {"choices":[{"delta":{"content":"สวัสดี"}}]}"#;
        assert_eq!(parse_openai_sse_line(line).as_deref(), Some("สวัสดี"));
        assert_eq!(parse_openai_sse_line("data: [DONE]"), None);
        assert_eq!(parse_openai_sse_line("event: ping"), None);
    }

    #[test]
    fn parses_gemini_sse_chunks() {
        let line = r#"data: {"candidates":[{"content":{"parts":[{"text":"Hello "},{"text":"world"}]}}]}"#;
        assert_eq!(parse_gemini_sse_line(line).as_deref(), Some("Hello world"));
    }

    #[test]
    fn parses_claude_sse_content_block_delta() {
        let delta = r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Bonjour"}}"#;
        let ping = r#"data: {"type":"ping"}"#;
        assert_eq!(parse_claude_sse_line(delta).as_deref(), Some("Bonjour"));
        assert_eq!(parse_claude_sse_line(ping), None);
    }
}

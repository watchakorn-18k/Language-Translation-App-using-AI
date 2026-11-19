use crate::providers::{ProviderError, streaming_client};
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

/// Parses a single OpenAI-compatible SSE `data:` line and returns the delta text if present.
pub fn parse_openai_sse_line(line: &str) -> Option<String> {
    let payload = line.trim().strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let val: Value = serde_json::from_str(payload).ok()?;
    val.pointer("/choices/0/delta/content")?
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
    let parts = val.pointer("/candidates/0/content/parts")?.as_array()?;
    let chunk: String = parts
        .iter()
        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
        .collect();
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
    val.pointer("/delta/text")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Splits a byte stream into complete lines. Bytes are buffered until a newline arrives, so a
/// multi-byte UTF-8 character (Thai, CJK, emoji) split across network chunks is never corrupted.
#[derive(Debug, Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            lines.push(String::from_utf8_lossy(&line).trim_end().to_string());
        }
        lines
    }

    /// Returns any trailing partial line once the stream has ended.
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let rest = String::from_utf8_lossy(&std::mem::take(&mut self.buf))
            .trim_end()
            .to_string();
        if rest.is_empty() { None } else { Some(rest) }
    }
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
    let mut lines = LineBuffer::default();
    let mut full = String::new();

    let handle = |line: &str, full: &mut String| {
        if let Some(delta) = parse_line(line) {
            full.push_str(&delta);
            let _ = tx.send(delta);
        }
    };

    while let Some(item) = stream.next().await {
        let bytes = item?;
        for line in lines.push(&bytes) {
            handle(&line, &mut full);
        }
    }
    if let Some(rest) = lines.finish() {
        handle(&rest, &mut full);
    }

    if full.is_empty() {
        Err(ProviderError::EmptyResponse)
    } else {
        Ok(full)
    }
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
    let payload = serde_json::json!({
        "model": model,
        "temperature": 0.1,
        "max_tokens": 2000,
        "stream": true,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ]
    });

    let mut req = streaming_client().post(&endpoint).json(&payload);
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.bearer_auth(key.trim());
    }

    let resp = req.send().await?;
    consume_sse_stream(resp, parse_openai_sse_line, tx).await
}

pub async fn stream_gemini_at(
    base_url: &str,
    api_key: &str,
    model: &str,
    full_prompt: &str,
    tx: &UnboundedSender<String>,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Gemini"));
    }
    let url = format!(
        "{}/v1beta/models/{model}:streamGenerateContent",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": full_prompt }] }]
    });

    let resp = streaming_client()
        .post(&url)
        .query(&[("key", api_key.trim()), ("alt", "sse")])
        .json(&body)
        .send()
        .await?;

    consume_sse_stream(resp, parse_gemini_sse_line, tx).await
}

pub async fn stream_claude_at(
    base_url: &str,
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

    let resp = streaming_client()
        .post(format!("{}/v1/messages", base_url.trim_end_matches('/')))
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
    use tokio::sync::mpsc::unbounded_channel;
    use wiremock::matchers::{header, method, path, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sse(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.to_string(), "text/event-stream")
    }

    fn drain(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(s) = rx.try_recv() {
            out.push(s);
        }
        out
    }

    #[test]
    fn parses_openai_sse_deltas_and_done() {
        let line = r#"data: {"choices":[{"delta":{"content":"สวัสดี"}}]}"#;
        assert_eq!(parse_openai_sse_line(line).as_deref(), Some("สวัสดี"));
        assert_eq!(parse_openai_sse_line("data: [DONE]"), None);
        assert_eq!(parse_openai_sse_line("event: ping"), None);
        assert_eq!(
            parse_openai_sse_line(r#"data: {"choices":[{"delta":{}}]}"#),
            None
        );
        assert_eq!(parse_openai_sse_line("data: not-json"), None);
    }

    #[test]
    fn parses_gemini_sse_chunks() {
        let line =
            r#"data: {"candidates":[{"content":{"parts":[{"text":"Hello "},{"text":"world"}]}}]}"#;
        assert_eq!(parse_gemini_sse_line(line).as_deref(), Some("Hello world"));
        assert_eq!(parse_gemini_sse_line(r#"data: {"candidates":[]}"#), None);
    }

    #[test]
    fn parses_claude_sse_content_block_delta() {
        let delta = r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Bonjour"}}"#;
        let ping = r#"data: {"type":"ping"}"#;
        assert_eq!(parse_claude_sse_line(delta).as_deref(), Some("Bonjour"));
        assert_eq!(parse_claude_sse_line(ping), None);
    }

    #[test]
    fn line_buffer_joins_lines_split_across_chunks() {
        let mut lb = LineBuffer::default();
        assert!(lb.push(b"data: {\"a\"").is_empty());
        let lines = lb.push(b": 1}\r\ndata: second\n");
        assert_eq!(
            lines,
            vec!["data: {\"a\": 1}".to_string(), "data: second".to_string()]
        );
        assert_eq!(lb.push(b"tail"), Vec::<String>::new());
        assert_eq!(lb.finish().as_deref(), Some("tail"));
        assert_eq!(lb.finish(), None);
    }

    #[test]
    fn line_buffer_never_corrupts_multibyte_characters_split_across_chunks() {
        let text = "data: สวัสดีชาวโลก 你好 😀\n";
        let bytes = text.as_bytes();
        // Feed one byte at a time: every multi-byte sequence is split.
        let mut lb = LineBuffer::default();
        let mut lines = Vec::new();
        for b in bytes {
            lines.extend(lb.push(&[*b]));
        }
        assert_eq!(lines, vec!["data: สวัสดีชาวโลก 你好 😀".to_string()]);
        assert!(!lines[0].contains('\u{FFFD}'));
    }

    #[tokio::test]
    async fn streams_openai_deltas_in_order_and_returns_full_text() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"สวัส\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ดี\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk"))
            .respond_with(sse(body))
            .expect(1)
            .mount(&server)
            .await;

        let (tx, mut rx) = unbounded_channel();
        let full = stream_openai_compatible(
            &format!("{}/v1", server.uri()),
            Some("sk"),
            "m",
            "sys",
            "user",
            &tx,
        )
        .await
        .unwrap();
        assert_eq!(full, "สวัสดี");
        assert_eq!(drain(&mut rx), vec!["สวัส".to_string(), "ดี".to_string()]);

        let requests = server.received_requests().await.unwrap();
        let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(sent["stream"], true);
    }

    #[tokio::test]
    async fn streams_gemini_chunks() {
        let server = MockServer::start().await;
        let body = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hel\"}]}}]}\n\n",
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"lo\"}]}}]}\n\n"
        );
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/g-test:streamGenerateContent$"))
            .and(query_param("alt", "sse"))
            .and(query_param("key", "AIza"))
            .respond_with(sse(body))
            .expect(1)
            .mount(&server)
            .await;

        let (tx, mut rx) = unbounded_channel();
        let full = stream_gemini_at(&server.uri(), "AIza", "g-test", "p", &tx)
            .await
            .unwrap();
        assert_eq!(full, "Hello");
        assert_eq!(drain(&mut rx), vec!["Hel".to_string(), "lo".to_string()]);
    }

    #[tokio::test]
    async fn streams_claude_text_deltas_and_ignores_other_events() {
        let server = MockServer::start().await;
        let body = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\"}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Bon\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"jour\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "k"))
            .respond_with(sse(body))
            .expect(1)
            .mount(&server)
            .await;

        let (tx, mut rx) = unbounded_channel();
        let full = stream_claude_at(&server.uri(), "k", "c", "sys", "user", &tx)
            .await
            .unwrap();
        assert_eq!(full, "Bonjour");
        assert_eq!(drain(&mut rx).concat(), "Bonjour");
    }

    #[tokio::test]
    async fn stream_reports_http_errors_and_empty_streams() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
            .mount(&server)
            .await;
        let (tx, _rx) = unbounded_channel();
        let err =
            stream_openai_compatible(&format!("{}/v1", server.uri()), None, "m", "s", "u", &tx)
                .await
                .unwrap_err();
        assert!(matches!(err, ProviderError::Api { status: 429, .. }));

        let empty = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(sse("data: [DONE]\n\n"))
            .mount(&empty)
            .await;
        let err =
            stream_openai_compatible(&format!("{}/v1", empty.uri()), None, "m", "s", "u", &tx)
                .await
                .unwrap_err();
        assert!(matches!(err, ProviderError::EmptyResponse));

        let err = stream_claude_at(&empty.uri(), "", "m", "s", "u", &tx)
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::MissingApiKey("Claude")));
        let err = stream_gemini_at(&empty.uri(), " ", "m", "p", &tx)
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::MissingApiKey("Gemini")));
    }
}

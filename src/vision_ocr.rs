use crate::glossary::GlossaryStore;
use crate::providers::ProviderError;
use crate::translator::TranslatorProvider;
use base64::Engine;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionOcrOutcome {
    pub source_text: String,
    pub primary_english: String,
    pub secondary_localized: String,
}

/// Encodes a PNG file to base64 for multimodal LLM requests.
pub fn encode_image_base64(path: &Path) -> Result<String, ProviderError> {
    let bytes = std::fs::read(path).map_err(|e| ProviderError::Api {
        status: 0,
        body: format!("Failed to read image `{}`: {e}", path.display()),
    })?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Parses structured JSON `{"source_text": "...", "english": "...", "target": "..."}`
/// returned by Vision AI OCR, with graceful fallback if the model emits plain text.
pub fn parse_vision_response(raw: &str) -> VisionOcrOutcome {
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
        if end >= start {
            if let Ok(val) = serde_json::from_str::<Value>(&raw[start..=end]) {
                let source = val
                    .get("source_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let english = val
                    .get("english")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let target = val
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !english.is_empty() || !target.is_empty() || !source.is_empty() {
                    return VisionOcrOutcome {
                        source_text: if source.is_empty() {
                            english.clone()
                        } else {
                            source
                        },
                        primary_english: english,
                        secondary_localized: target,
                    };
                }
            }
        }
    }

    let trimmed = raw.trim().to_string();
    VisionOcrOutcome {
        source_text: trimmed.clone(),
        primary_english: trimmed.clone(),
        secondary_localized: trimmed,
    }
}

/// Sends a cropped image directly to a multimodal AI provider (OpenAI, Gemini, Claude,
/// Ollama, or Custom OpenAI-compatible) to extract text and translate it in a single request.
pub async fn vision_ocr_and_translate(
    image_path: &Path,
    provider: TranslatorProvider,
    target_language: &str,
) -> Result<VisionOcrOutcome, ProviderError> {
    let b64 = encode_image_base64(image_path)?;
    let glossary = GlossaryStore::default().format_prompt_section();
    let prompt = format!(
        "Read all visible text from this image accurately, then translate it into English and into {target_language}. \
         Return ONLY valid JSON with keys \"source_text\", \"english\", and \"target\".{glossary}"
    );

    let cfg = provider.load_settings();
    let client = reqwest::Client::new();

    let raw_text = match provider {
        TranslatorProvider::OpenAi
        | TranslatorProvider::Ollama
        | TranslatorProvider::CustomOpenAi => {
            let base = if provider == TranslatorProvider::OpenAi {
                "https://api.openai.com/v1"
            } else {
                cfg.base_url.as_str()
            };
            if provider == TranslatorProvider::OpenAi && cfg.api_key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey("OpenAI"));
            }
            let endpoint = format!("{}/chat/completions", base.trim_end_matches('/'));
            let data_uri = format!("data:image/png;base64,{b64}");
            let payload = serde_json::json!({
                "model": cfg.model,
                "temperature": 0.1,
                "max_tokens": 2000,
                "messages": [{
                    "role": "user",
                    "content": [
                        { "type": "text", "text": prompt },
                        { "type": "image_url", "image_url": { "url": data_uri } }
                    ]
                }]
            });
            let mut req = client.post(&endpoint).json(&payload);
            if !cfg.api_key.trim().is_empty() {
                req = req.bearer_auth(cfg.api_key.trim());
            }
            let resp = req.send().await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(ProviderError::Api {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let val: Value = resp.json().await?;
            val.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
                .ok_or(ProviderError::EmptyResponse)?
                .to_string()
        }
        TranslatorProvider::Gemini => {
            if cfg.api_key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey("Gemini"));
            }
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                cfg.model
            );
            let payload = serde_json::json!({
                "contents": [{
                    "parts": [
                        { "text": prompt },
                        { "inline_data": { "mime_type": "image/png", "data": b64 } }
                    ]
                }]
            });
            let resp = client
                .post(&url)
                .query(&[("key", cfg.api_key.trim())])
                .json(&payload)
                .send()
                .await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(ProviderError::Api {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let val: Value = resp.json().await?;
            val.get("candidates")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .and_then(|a| a.iter().find_map(|p| p.get("text").and_then(|t| t.as_str())))
                .ok_or(ProviderError::EmptyResponse)?
                .to_string()
        }
        TranslatorProvider::Claude => {
            if cfg.api_key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey("Claude"));
            }
            let payload = serde_json::json!({
                "model": cfg.model,
                "max_tokens": 2000,
                "messages": [{
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": b64
                            }
                        },
                        { "type": "text", "text": prompt }
                    ]
                }]
            });
            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", cfg.api_key.trim())
                .header("anthropic-version", "2023-06-01")
                .json(&payload)
                .send()
                .await?;
            let status = resp.status();
            if !status.is_success() {
                return Err(ProviderError::Api {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            let val: Value = resp.json().await?;
            val.get("content")
                .and_then(|c| c.as_array())
                .and_then(|a| a.iter().find_map(|b| b.get("text").and_then(|t| t.as_str())))
                .ok_or(ProviderError::EmptyResponse)?
                .to_string()
        }
        TranslatorProvider::GoogleTranslate | TranslatorProvider::DeepSeek => {
            return Err(ProviderError::Api {
                status: 400,
                body: format!("{} does not support Vision AI OCR", provider.label()),
            });
        }
    };

    Ok(parse_vision_response(&raw_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_vision_json_with_unicode() {
        let raw = "```json\n{\"source_text\":\"こんにちは\",\"english\":\"Hello\",\"target\":\"สวัสดี\"}\n```";
        let out = parse_vision_response(raw);
        assert_eq!(out.source_text, "こんにちは");
        assert_eq!(out.primary_english, "Hello");
        assert_eq!(out.secondary_localized, "สวัสดี");
    }

    #[test]
    fn falls_back_gracefully_on_plain_text() {
        let out = parse_vision_response("  Plain OCR result  ");
        assert_eq!(out.source_text, "Plain OCR result");
        assert_eq!(out.primary_english, "Plain OCR result");
        assert_eq!(out.secondary_localized, "Plain OCR result");
    }
}

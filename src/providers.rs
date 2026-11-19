use crate::apikey::{ApikeyGpt, ChatGptConfig, DEFAULT_CHATGPT_CONFIG_PATH};
use crate::gemini::{DEFAULT_GEMINI_CONFIG_PATH, GeminiConfig};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

pub const DEFAULT_CLAUDE_CONFIG_PATH: &str = "api_claude.json";
pub const DEFAULT_DEEPSEEK_CONFIG_PATH: &str = "api_deepseek.json";
pub const DEFAULT_OLLAMA_CONFIG_PATH: &str = "ollama_config.json";
pub const DEFAULT_CUSTOM_CONFIG_PATH: &str = "custom_openai.json";

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-6.2-sol";
pub const DEFAULT_GEMINI_MODEL_MODERN: &str = "gemini-4-neon";
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-5-5";
pub const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma4";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
pub const DEFAULT_CUSTOM_MODEL: &str = "gpt-6.2-sol";
pub const DEFAULT_CUSTOM_BASE_URL: &str = "https://openrouter.ai/api/v1";

const TRANSLATION_SYSTEM_PROMPT: &str = "You are a translation expert. Translate the input text into natural, accurate English while preserving the original meaning, tone, and proper nouns (brand names, company names, technical terms). Return only the translated English text without commentary.";

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API returned status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("Missing API key for {0}")]
    MissingApiKey(&'static str),
    #[error("Empty response from provider")]
    EmptyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSettings {
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub base_url: String,
}

pub fn load_provider_settings(path: &str, default_model: &str, default_base_url: &str) -> ProviderSettings {
    if let Ok(content) = fs::read_to_string(path) {
        if let Ok(mut parsed) = serde_json::from_str::<ProviderSettings>(&content) {
            if parsed.model.trim().is_empty() {
                parsed.model = default_model.to_string();
            }
            if parsed.base_url.trim().is_empty() {
                parsed.base_url = default_base_url.to_string();
            }
            return parsed;
        }
    }
    ProviderSettings {
        api_key: String::new(),
        model: default_model.to_string(),
        base_url: default_base_url.to_string(),
    }
}

pub fn save_provider_settings(path: &str, settings: &ProviderSettings) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(settings).unwrap_or_default();
    fs::write(path, json)
}

pub fn load_openai_settings() -> ProviderSettings {
    let mgr = ApikeyGpt::default();
    let cfg = mgr.load_config().unwrap_or_default();
    let api_key = if cfg.api_key == crate::apikey::DEFAULT_CHATGPT_API_KEY_PLACEHOLDER {
        String::new()
    } else {
        cfg.api_key
    };
    let model = if cfg.model.trim().is_empty() {
        DEFAULT_OPENAI_MODEL.to_string()
    } else {
        cfg.model
    };
    ProviderSettings {
        api_key,
        model,
        base_url: "https://api.openai.com/v1".to_string(),
    }
}

pub fn save_openai_settings(api_key: &str, model: &str) -> std::io::Result<()> {
    let cfg = ChatGptConfig {
        api_key: api_key.trim().to_string(),
        model: if model.trim().is_empty() {
            DEFAULT_OPENAI_MODEL.to_string()
        } else {
            model.trim().to_string()
        },
    };
    let json = serde_json::to_string_pretty(&cfg).unwrap_or_default();
    fs::write(Path::new(DEFAULT_CHATGPT_CONFIG_PATH), json)
}

pub fn load_gemini_settings() -> ProviderSettings {
    if let Ok(content) = fs::read_to_string(DEFAULT_GEMINI_CONFIG_PATH) {
        if let Ok(cfg) = serde_json::from_str::<GeminiConfig>(&content) {
            let api_key = if cfg.api_key == crate::gemini::DEFAULT_GEMINI_API_KEY_PLACEHOLDER {
                String::new()
            } else {
                cfg.api_key
            };
            let model = if cfg.model.trim().is_empty() {
                DEFAULT_GEMINI_MODEL_MODERN.to_string()
            } else {
                cfg.model
            };
            return ProviderSettings {
                api_key,
                model,
                base_url: String::new(),
            };
        }
    }
    ProviderSettings {
        api_key: String::new(),
        model: DEFAULT_GEMINI_MODEL_MODERN.to_string(),
        base_url: String::new(),
    }
}

pub fn save_gemini_settings(api_key: &str, model: &str) -> std::io::Result<()> {
    let cfg = GeminiConfig {
        api_key: api_key.trim().to_string(),
        model: if model.trim().is_empty() {
            DEFAULT_GEMINI_MODEL_MODERN.to_string()
        } else {
            model.trim().to_string()
        },
    };
    let json = serde_json::to_string_pretty(&cfg).unwrap_or_default();
    fs::write(Path::new(DEFAULT_GEMINI_CONFIG_PATH), json)
}

#[derive(Debug, Serialize)]
struct OpenAiCompatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<OpenAiCompatMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAiCompatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatResponse {
    choices: Vec<OpenAiCompatChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatChoice {
    message: OpenAiCompatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompatChoiceMessage {
    content: Option<String>,
}

/// Calls any OpenAI-compatible `/chat/completions` endpoint (DeepSeek, Ollama, OpenRouter, Groq, LM Studio).
pub async fn translate_openai_compatible(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    text: &str,
) -> Result<String, ProviderError> {
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let payload = OpenAiCompatRequest {
        model,
        temperature: 0.1,
        max_tokens: 2000,
        messages: vec![
            OpenAiCompatMessage {
                role: "system",
                content: TRANSLATION_SYSTEM_PROMPT.to_string(),
            },
            OpenAiCompatMessage {
                role: "user",
                content: text.to_string(),
            },
        ],
    };

    let client = reqwest::Client::new();
    let mut req = client.post(&endpoint).json(&payload);
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.bearer_auth(key);
    }

    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ProviderError::Api {
            status: status.as_u16(),
            body,
        });
    }

    let parsed: OpenAiCompatResponse = resp.json().await?;
    parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .ok_or(ProviderError::EmptyResponse)
}

#[derive(Debug, Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'static str,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    #[serde(default)]
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
}

/// Calls the Anthropic Messages API (`https://api.anthropic.com/v1/messages`).
pub async fn translate_claude(
    api_key: &str,
    model: &str,
    text: &str,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Claude"));
    }

    let payload = ClaudeRequest {
        model,
        max_tokens: 2000,
        system: TRANSLATION_SYSTEM_PROMPT,
        messages: vec![ClaudeMessage {
            role: "user",
            content: text.to_string(),
        }],
    };

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ProviderError::Api {
            status: status.as_u16(),
            body,
        });
    }

    let parsed: ClaudeResponse = resp.json().await?;
    parsed
        .content
        .into_iter()
        .find_map(|b| b.text)
        .ok_or(ProviderError::EmptyResponse)
}

pub const APP_SETTINGS_PATH: &str = "app_settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPreferences {
    pub translator: String,
    pub target_language: String,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            translator: "Google Translate".to_string(),
            target_language: "thai".to_string(),
        }
    }
}

pub fn load_app_preferences() -> AppPreferences {
    if let Ok(content) = fs::read_to_string(APP_SETTINGS_PATH) {
        if let Ok(prefs) = serde_json::from_str::<AppPreferences>(&content) {
            return prefs;
        }
    }
    AppPreferences::default()
}

pub fn save_app_preferences(translator: &str, target_language: &str) {
    let prefs = AppPreferences {
        translator: translator.to_string(),
        target_language: target_language.to_string(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&prefs) {
        let _ = fs::write(APP_SETTINGS_PATH, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_provider_settings_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_claude.json");
        let path_str = path.to_str().unwrap();

        let cfg = ProviderSettings {
            api_key: "sk-ant-test-123".to_string(),
            model: DEFAULT_CLAUDE_MODEL.to_string(),
            base_url: String::new(),
        };
        save_provider_settings(path_str, &cfg).unwrap();
        let loaded = load_provider_settings(path_str, DEFAULT_CLAUDE_MODEL, "");
        assert_eq!(loaded.api_key, "sk-ant-test-123");
        assert_eq!(loaded.model, DEFAULT_CLAUDE_MODEL);
    }
}

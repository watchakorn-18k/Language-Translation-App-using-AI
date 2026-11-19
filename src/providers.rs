use crate::apikey::{ApikeyGpt, ChatGptConfig, DEFAULT_CHATGPT_CONFIG_PATH};
use crate::gemini::{DEFAULT_GEMINI_CONFIG_PATH, GeminiConfig};
use crate::glossary::GlossaryStore;
use crate::secrets::SecretStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

pub const APP_SETTINGS_PATH: &str = "app_settings.json";

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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPreferences {
    pub translator: String,
    pub target_language: String,
    #[serde(default = "default_true")]
    pub auto_translate_after_ocr: bool,
    #[serde(default = "default_true")]
    pub direct_target_translation: bool,
    #[serde(default)]
    pub vision_ocr: bool,
    #[serde(default = "default_true")]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub streaming_enabled: bool,
    #[serde(default = "default_true")]
    pub use_keychain: bool,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            translator: "Google Translate".to_string(),
            target_language: "thai".to_string(),
            auto_translate_after_ocr: true,
            direct_target_translation: true,
            vision_ocr: false,
            always_on_top: true,
            streaming_enabled: true,
            use_keychain: true,
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

pub fn save_app_preferences(prefs: &AppPreferences) {
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        let _ = fs::write(APP_SETTINGS_PATH, json);
    }
}

/// Builds the system prompt for English-only translation (with glossary injected).
pub fn build_english_system_prompt() -> String {
    let glossary = GlossaryStore::default().format_prompt_section();
    format!(
        "You are a translation expert. Translate the input text into natural, accurate English \
         while preserving the original meaning, tone, and proper nouns (brand names, company names, \
         technical terms). Return only the translated English text without commentary.{glossary}"
    )
}

/// Builds the system prompt for Single-Hop bilingual translation (English + target language in 1 call).
pub fn build_bilingual_system_prompt(target_language: &str) -> String {
    let glossary = GlossaryStore::default().format_prompt_section();
    format!(
        "You are a bilingual translation expert. Translate the input text into both natural English \
         and natural {target_language}, preserving tone and proper nouns. \
         Return ONLY valid JSON with exact keys \"english\" and \"target\" and no extra text.{glossary}"
    )
}

/// Parses a single-hop bilingual JSON response `{"english": "...", "target": "..."}`.
/// Returns `None` if the text is not valid bilingual JSON.
pub fn parse_bilingual_response(raw: &str) -> Option<(String, String)> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end < start {
        return None;
    }
    let val: Value = serde_json::from_str(&raw[start..=end]).ok()?;
    let english = val.get("english")?.as_str()?.trim().to_string();
    let target = val.get("target")?.as_str()?.trim().to_string();
    if english.is_empty() && target.is_empty() {
        return None;
    }
    Some((english, target))
}

pub fn load_provider_settings(
    path: &str,
    provider_key: &str,
    default_model: &str,
    default_base_url: &str,
    use_keychain: bool,
) -> ProviderSettings {
    let mut settings = if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str::<ProviderSettings>(&content).unwrap_or(ProviderSettings {
            api_key: String::new(),
            model: default_model.to_string(),
            base_url: default_base_url.to_string(),
        })
    } else {
        ProviderSettings {
            api_key: String::new(),
            model: default_model.to_string(),
            base_url: default_base_url.to_string(),
        }
    };

    if settings.model.trim().is_empty() {
        settings.model = default_model.to_string();
    }
    if settings.base_url.trim().is_empty() {
        settings.base_url = default_base_url.to_string();
    }

    if use_keychain {
        if let Some(secret) = SecretStore::get_secret(provider_key) {
            settings.api_key = secret;
        }
    }

    settings
}

pub fn save_provider_settings(
    path: &str,
    provider_key: &str,
    settings: &ProviderSettings,
    use_keychain: bool,
) -> std::io::Result<()> {
    let mut to_write = settings.clone();
    if use_keychain && SecretStore::set_secret(provider_key, &settings.api_key) {
        // Key stored safely in OS Keychain; omit plaintext secret from JSON file.
        to_write.api_key = String::new();
    }
    let json = serde_json::to_string_pretty(&to_write).unwrap_or_default();
    fs::write(path, json)
}

pub fn load_openai_settings(use_keychain: bool) -> ProviderSettings {
    let mgr = ApikeyGpt::default();
    let cfg = mgr.load_config().unwrap_or_default();
    let mut api_key = if cfg.api_key == crate::apikey::DEFAULT_CHATGPT_API_KEY_PLACEHOLDER {
        String::new()
    } else {
        cfg.api_key
    };
    if use_keychain {
        if let Some(secret) = SecretStore::get_secret("openai") {
            api_key = secret;
        }
    }
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

pub fn save_openai_settings(api_key: &str, model: &str, use_keychain: bool) -> std::io::Result<()> {
    let stored_in_keychain = use_keychain && SecretStore::set_secret("openai", api_key);
    let cfg = ChatGptConfig {
        api_key: if stored_in_keychain {
            String::new()
        } else {
            api_key.trim().to_string()
        },
        model: if model.trim().is_empty() {
            DEFAULT_OPENAI_MODEL.to_string()
        } else {
            model.trim().to_string()
        },
    };
    let json = serde_json::to_string_pretty(&cfg).unwrap_or_default();
    fs::write(Path::new(DEFAULT_CHATGPT_CONFIG_PATH), json)
}

pub fn load_gemini_settings(use_keychain: bool) -> ProviderSettings {
    let mut api_key = String::new();
    let mut model = DEFAULT_GEMINI_MODEL_MODERN.to_string();

    if let Ok(content) = fs::read_to_string(DEFAULT_GEMINI_CONFIG_PATH) {
        if let Ok(cfg) = serde_json::from_str::<GeminiConfig>(&content) {
            if cfg.api_key != crate::gemini::DEFAULT_GEMINI_API_KEY_PLACEHOLDER {
                api_key = cfg.api_key;
            }
            if !cfg.model.trim().is_empty() {
                model = cfg.model;
            }
        }
    }

    if use_keychain {
        if let Some(secret) = SecretStore::get_secret("gemini") {
            api_key = secret;
        }
    }

    ProviderSettings {
        api_key,
        model,
        base_url: String::new(),
    }
}

pub fn save_gemini_settings(api_key: &str, model: &str, use_keychain: bool) -> std::io::Result<()> {
    let stored_in_keychain = use_keychain && SecretStore::set_secret("gemini", api_key);
    let cfg = GeminiConfig {
        api_key: if stored_in_keychain {
            String::new()
        } else {
            api_key.trim().to_string()
        },
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

/// Calls any OpenAI-compatible `/chat/completions` endpoint (OpenAI, DeepSeek, Ollama, OpenRouter, Groq, LM Studio).
pub async fn translate_openai_compatible(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system_prompt: &str,
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
                content: system_prompt.to_string(),
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
    system: &'a str,
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
    system_prompt: &str,
    text: &str,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Claude"));
    }

    let payload = ClaudeRequest {
        model,
        max_tokens: 2000,
        system: system_prompt,
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

/// Calls Google Gemini `generateContent` directly with a custom prompt.
pub async fn translate_gemini_prompt(
    api_key: &str,
    model: &str,
    full_prompt: &str,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Gemini"));
    }
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": full_prompt }] }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .query(&[("key", api_key.trim())])
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(ProviderError::Api {
            status: status.as_u16(),
            body: text,
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
        .map(|s| s.to_string())
        .ok_or(ProviderError::EmptyResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_provider_settings_roundtrip_without_keychain() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_claude.json");
        let path_str = path.to_str().unwrap();

        let cfg = ProviderSettings {
            api_key: "sk-ant-test-123".to_string(),
            model: DEFAULT_CLAUDE_MODEL.to_string(),
            base_url: String::new(),
        };
        save_provider_settings(path_str, "claude_test", &cfg, false).unwrap();
        let loaded = load_provider_settings(path_str, "claude_test", DEFAULT_CLAUDE_MODEL, "", false);
        assert_eq!(loaded.api_key, "sk-ant-test-123");
        assert_eq!(loaded.model, DEFAULT_CLAUDE_MODEL);
    }

    #[test]
    fn parses_single_hop_bilingual_json() {
        let raw = "```json\n{\"english\": \"Good morning\", \"target\": \"อรุณสวัสดิ์\"}\n```";
        let (en, th) = parse_bilingual_response(raw).unwrap();
        assert_eq!(en, "Good morning");
        assert_eq!(th, "อรุณสวัสดิ์");
    }

    #[test]
    fn app_preferences_defaults_deserialize_from_legacy_json() {
        let legacy = r#"{"translator":"OpenAI","target_language":"japanese"}"#;
        let prefs: AppPreferences = serde_json::from_str(legacy).unwrap();
        assert_eq!(prefs.translator, "OpenAI");
        assert_eq!(prefs.target_language, "japanese");
        assert!(prefs.auto_translate_after_ocr);
        assert!(prefs.direct_target_translation);
        assert!(prefs.streaming_enabled);
        assert!(prefs.use_keychain);
        assert!(!prefs.vision_ocr);
    }
}

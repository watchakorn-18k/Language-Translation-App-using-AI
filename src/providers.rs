use crate::apikey::{self, ChatGptConfig};
use crate::gemini::{self, GeminiConfig};
use crate::paths;
use crate::secrets::SecretStore;
use crate::theme::ThemeChoice;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_CLAUDE_CONFIG_PATH: &str = "api_claude.json";
pub const DEFAULT_DEEPSEEK_CONFIG_PATH: &str = "api_deepseek.json";
pub const DEFAULT_OLLAMA_CONFIG_PATH: &str = "ollama_config.json";
pub const DEFAULT_CUSTOM_CONFIG_PATH: &str = "custom_openai.json";
pub const APP_SETTINGS_PATH: &str = "app_settings.json";

pub const DEFAULT_OPENAI_MODEL: &str = "gpt-6.2-sol";
pub const DEFAULT_GEMINI_MODEL_MODERN: &str = "gemini-4-neon";
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-5-5";
pub const DEFAULT_DEEPSEEK_MODEL: &str = "deepseek-chat";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma4";
pub const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
pub const DEFAULT_CUSTOM_MODEL: &str = "gpt-6.2-sol";
pub const DEFAULT_CUSTOM_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Base URLs of every remote service. Injectable so the whole pipeline can be tested against a mock server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    pub openai: String,
    pub deepseek: String,
    pub anthropic: String,
    pub gemini: String,
    pub google_translate: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            openai: "https://api.openai.com/v1".to_string(),
            deepseek: "https://api.deepseek.com/v1".to_string(),
            anthropic: "https://api.anthropic.com".to_string(),
            gemini: "https://generativelanguage.googleapis.com".to_string(),
            google_translate: "https://translate.googleapis.com/translate_a/single".to_string(),
        }
    }
}

impl Endpoints {
    /// Points every service at one mock server base URL (OpenAI/DeepSeek use `/v1`).
    pub fn all_at(base: &str) -> Self {
        let base = base.trim_end_matches('/');
        Self {
            openai: format!("{base}/v1"),
            deepseek: format!("{base}/v1"),
            anthropic: base.to_string(),
            gemini: base.to_string(),
            google_translate: format!("{base}/translate_a/single"),
        }
    }
}

/// Client for regular request/response calls (bounded total time).
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
        .unwrap_or_default()
}

/// Client for streaming calls: bounded connect + per-read idle time, no total deadline.
pub fn streaming_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(60))
        .build()
        .unwrap_or_default()
}

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

/// Pulls a human readable message out of the JSON error bodies used by OpenAI, Anthropic and Gemini.
pub fn extract_api_error_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body.trim()).ok()?;
    let message = value
        .pointer("/error/message")
        .or_else(|| value.get("message"))
        .or_else(|| value.get("error"))
        .and_then(|v| v.as_str())?
        .trim();
    if message.is_empty() {
        return None;
    }
    let mut out: String = message.chars().take(200).collect();
    if message.chars().count() > 200 {
        out.push('…');
    }
    Some(out)
}

/// Turns a provider failure into one short, actionable sentence for the UI.
pub fn friendly_error(provider: &str, err: &ProviderError) -> String {
    match err {
        ProviderError::MissingApiKey(_) => {
            format!("Missing {provider} API key. Add it in Settings.")
        }
        ProviderError::EmptyResponse => format!("{provider} returned an empty response."),
        ProviderError::Http(e) => {
            if e.is_timeout() {
                format!("{provider} took too long to respond. Try again.")
            } else if e.is_connect() {
                if provider.starts_with("Ollama") {
                    "Cannot reach Ollama. Is it running? Start it with `ollama serve`.".to_string()
                } else {
                    format!("Cannot reach {provider}. Check your internet connection.")
                }
            } else {
                format!("Network error talking to {provider}.")
            }
        }
        ProviderError::Api { status, body } => {
            let detail = extract_api_error_message(body);
            let base = match *status {
                401 | 403 => {
                    format!(
                        "{provider} rejected the API key (HTTP {status}). Check it in Settings."
                    )
                }
                404 => format!(
                    "{provider} could not find that model or endpoint (HTTP 404). Check the model name in Settings."
                ),
                429 => format!(
                    "{provider} rate limit or quota reached (HTTP 429). Wait a moment or check your plan."
                ),
                500..=599 => {
                    format!(
                        "{provider} is temporarily unavailable (HTTP {status}). Try again shortly."
                    )
                }
                _ => format!("{provider} rejected the request (HTTP {status})."),
            };
            match detail {
                Some(d) => format!("{base} {d}"),
                None => base,
            }
        }
    }
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

fn default_ocr_language() -> String {
    "Chinese and English".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPreferences {
    pub translator: String,
    pub target_language: String,
    #[serde(default = "default_ocr_language")]
    pub ocr_language: String,
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
    #[serde(default)]
    pub translation_overlay: bool,
    #[serde(default)]
    pub clipboard_watch: bool,
    #[serde(default)]
    pub theme: ThemeChoice,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            translator: "Google Translate".to_string(),
            target_language: "thai".to_string(),
            ocr_language: default_ocr_language(),
            auto_translate_after_ocr: true,
            direct_target_translation: true,
            vision_ocr: false,
            always_on_top: true,
            streaming_enabled: true,
            use_keychain: true,
            translation_overlay: false,
            clipboard_watch: false,
            theme: ThemeChoice::System,
        }
    }
}

pub fn load_preferences_from(path: &Path) -> AppPreferences {
    fs::read_to_string(path)
        .ok()
        .and_then(|c| serde_json::from_str::<AppPreferences>(&c).ok())
        .unwrap_or_default()
}

pub fn save_preferences_to(path: &Path, prefs: &AppPreferences) {
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        let _ = fs::write(path, json);
    }
}

pub fn load_app_preferences() -> AppPreferences {
    load_preferences_from(&paths::config_file(APP_SETTINGS_PATH))
}

pub fn save_app_preferences(prefs: &AppPreferences) {
    save_preferences_to(&paths::config_file(APP_SETTINGS_PATH), prefs);
}

/// System prompt for English-only translation. `glossary` is a pre-formatted prompt section (may be empty).
pub fn build_english_system_prompt(glossary: &str) -> String {
    format!(
        "You are a translation expert. Translate the input text into natural, accurate English \
         while preserving the original meaning, tone, and proper nouns (brand names, company names, \
         technical terms). Return only the translated English text without commentary.{glossary}"
    )
}

/// System prompt for single-hop bilingual translation (English + target language in one call).
pub fn build_bilingual_system_prompt(target_language: &str, glossary: &str) -> String {
    format!(
        "You are a bilingual translation expert. Translate the input text into both natural English \
         and natural {target_language}, preserving tone and proper nouns. \
         Return ONLY valid JSON with exact keys \"english\" and \"target\" and no extra text.{glossary}"
    )
}

/// Parses a single-hop bilingual JSON response `{"english": "...", "target": "..."}`.
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

fn blank_model_to(default_model: &str, model: &str) -> String {
    if model.trim().is_empty() {
        default_model.to_string()
    } else {
        model.trim().to_string()
    }
}

pub fn load_provider_settings(
    path: impl AsRef<Path>,
    keychain_account: &str,
    default_model: &str,
    default_base_url: &str,
    use_keychain: bool,
) -> ProviderSettings {
    let mut settings = fs::read_to_string(path.as_ref())
        .ok()
        .and_then(|c| serde_json::from_str::<ProviderSettings>(&c).ok())
        .unwrap_or(ProviderSettings {
            api_key: String::new(),
            model: default_model.to_string(),
            base_url: default_base_url.to_string(),
        });

    settings.model = blank_model_to(default_model, &settings.model);
    if settings.base_url.trim().is_empty() {
        settings.base_url = default_base_url.to_string();
    }
    if use_keychain {
        if let Some(secret) = SecretStore::get_secret(keychain_account) {
            settings.api_key = secret;
        }
    }
    settings
}

pub fn save_provider_settings(
    path: impl AsRef<Path>,
    keychain_account: &str,
    settings: &ProviderSettings,
    use_keychain: bool,
) -> std::io::Result<()> {
    let mut to_write = settings.clone();
    to_write.api_key = to_write.api_key.trim().to_string();
    if use_keychain && SecretStore::set_secret(keychain_account, &to_write.api_key) {
        // Stored safely in the OS keychain; keep the secret out of the JSON file.
        to_write.api_key = String::new();
    }
    let json = serde_json::to_string_pretty(&to_write).map_err(std::io::Error::other)?;
    fs::write(path.as_ref(), json)
}

pub fn load_openai_settings(use_keychain: bool) -> ProviderSettings {
    let cfg = apikey::read_config(&paths::config_file(apikey::DEFAULT_CHATGPT_CONFIG_PATH))
        .unwrap_or_default();
    let mut api_key = apikey::clean_api_key(&cfg.api_key);
    if use_keychain {
        if let Some(secret) = SecretStore::get_secret("openai") {
            api_key = secret;
        }
    }
    ProviderSettings {
        api_key,
        model: blank_model_to(DEFAULT_OPENAI_MODEL, &cfg.model),
        base_url: Endpoints::default().openai,
    }
}

pub fn save_openai_settings(api_key: &str, model: &str, use_keychain: bool) -> std::io::Result<()> {
    let stored = use_keychain && SecretStore::set_secret("openai", api_key);
    let cfg = ChatGptConfig {
        api_key: if stored {
            String::new()
        } else {
            api_key.trim().to_string()
        },
        model: blank_model_to(DEFAULT_OPENAI_MODEL, model),
    };
    apikey::write_config(
        &paths::config_file(apikey::DEFAULT_CHATGPT_CONFIG_PATH),
        &cfg,
    )
}

pub fn load_gemini_settings(use_keychain: bool) -> ProviderSettings {
    let cfg = gemini::read_config(&paths::config_file(gemini::DEFAULT_GEMINI_CONFIG_PATH))
        .unwrap_or_default();
    let mut api_key = gemini::clean_api_key(&cfg.api_key);
    if use_keychain {
        if let Some(secret) = SecretStore::get_secret("gemini") {
            api_key = secret;
        }
    }
    ProviderSettings {
        api_key,
        model: blank_model_to(DEFAULT_GEMINI_MODEL_MODERN, &cfg.model),
        base_url: String::new(),
    }
}

pub fn save_gemini_settings(api_key: &str, model: &str, use_keychain: bool) -> std::io::Result<()> {
    let stored = use_keychain && SecretStore::set_secret("gemini", api_key);
    let cfg = GeminiConfig {
        api_key: if stored {
            String::new()
        } else {
            api_key.trim().to_string()
        },
        model: blank_model_to(DEFAULT_GEMINI_MODEL_MODERN, model),
    };
    gemini::write_config(
        &paths::config_file(gemini::DEFAULT_GEMINI_CONFIG_PATH),
        &cfg,
    )
}

#[derive(Debug, Serialize)]
struct OpenAiCompatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<OpenAiCompatMessage<'a>>,
}

#[derive(Debug, Serialize)]
struct OpenAiCompatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

fn check_status_blocking_body(
    status: reqwest::StatusCode,
    body: String,
) -> Result<(), ProviderError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(ProviderError::Api {
            status: status.as_u16(),
            body,
        })
    }
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
                content: system_prompt,
            },
            OpenAiCompatMessage {
                role: "user",
                content: text,
            },
        ],
    };

    let mut req = http_client().post(&endpoint).json(&payload);
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.bearer_auth(key.trim());
    }

    let resp = req.send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    check_status_blocking_body(status, body.clone())?;

    let val: Value = serde_json::from_str(&body).map_err(|_| ProviderError::EmptyResponse)?;
    val.pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or(ProviderError::EmptyResponse)
}

/// Calls the Anthropic Messages API at `{base_url}/v1/messages`.
pub async fn translate_claude_at(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    text: &str,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Claude"));
    }

    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 2000,
        "system": system_prompt,
        "messages": [{ "role": "user", "content": text }]
    });

    let resp = http_client()
        .post(format!("{}/v1/messages", base_url.trim_end_matches('/')))
        .header("x-api-key", api_key.trim())
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    check_status_blocking_body(status, body.clone())?;

    let val: Value = serde_json::from_str(&body).map_err(|_| ProviderError::EmptyResponse)?;
    val.get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| {
            a.iter()
                .find_map(|b| b.get("text").and_then(|t| t.as_str()))
        })
        .map(|s| s.to_string())
        .ok_or(ProviderError::EmptyResponse)
}

/// Calls Gemini `generateContent` at `{base_url}/v1beta/models/{model}:generateContent`.
pub async fn translate_gemini_prompt_at(
    base_url: &str,
    api_key: &str,
    model: &str,
    full_prompt: &str,
) -> Result<String, ProviderError> {
    if api_key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey("Gemini"));
    }
    let url = format!(
        "{}/v1beta/models/{model}:generateContent",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": full_prompt }] }]
    });

    let resp = http_client()
        .post(&url)
        .query(&[("key", api_key.trim())])
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    check_status_blocking_body(status, text.clone())?;

    let val: Value = serde_json::from_str(&text).map_err(|_| ProviderError::EmptyResponse)?;
    val.pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
        .and_then(|a| {
            a.iter()
                .find_map(|p| p.get("text").and_then(|t| t.as_str()))
        })
        .map(|s| s.to_string())
        .ok_or(ProviderError::EmptyResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiremock::matchers::{body_partial_json, header, method, path, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn saves_and_loads_provider_settings_roundtrip_without_keychain() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_claude.json");

        let cfg = ProviderSettings {
            api_key: "  sk-ant-test-123 ".to_string(),
            model: DEFAULT_CLAUDE_MODEL.to_string(),
            base_url: String::new(),
        };
        save_provider_settings(&path, "claude_test", &cfg, false).unwrap();
        let loaded = load_provider_settings(&path, "claude_test", DEFAULT_CLAUDE_MODEL, "", false);
        assert_eq!(loaded.api_key, "sk-ant-test-123");
        assert_eq!(loaded.model, DEFAULT_CLAUDE_MODEL);
    }

    #[test]
    fn missing_or_corrupt_provider_file_yields_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ollama.json");
        let loaded =
            load_provider_settings(&path, "x", "gemma4", "http://localhost:11434/v1", false);
        assert_eq!(loaded.model, "gemma4");
        assert_eq!(loaded.base_url, "http://localhost:11434/v1");
        assert!(loaded.api_key.is_empty());

        fs::write(&path, "{ not json").unwrap();
        let loaded =
            load_provider_settings(&path, "x", "gemma4", "http://localhost:11434/v1", false);
        assert_eq!(loaded.model, "gemma4");
    }

    #[test]
    fn blank_model_and_base_url_fall_back_to_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("custom.json");
        fs::write(&path, r#"{"api_key":"k","model":"  ","base_url":""}"#).unwrap();
        let loaded = load_provider_settings(&path, "x", "m", "http://b", false);
        assert_eq!(loaded.model, "m");
        assert_eq!(loaded.base_url, "http://b");
        assert_eq!(loaded.api_key, "k");
    }

    #[test]
    fn parses_single_hop_bilingual_json() {
        let raw = "```json\n{\"english\": \"Good morning\", \"target\": \"อรุณสวัสดิ์\"}\n```";
        let (en, th) = parse_bilingual_response(raw).unwrap();
        assert_eq!(en, "Good morning");
        assert_eq!(th, "อรุณสวัสดิ์");
        assert!(parse_bilingual_response("no json here").is_none());
        assert!(parse_bilingual_response(r#"{"english":"","target":""}"#).is_none());
        assert!(parse_bilingual_response(r#"{"english":"only"}"#).is_none());
    }

    #[test]
    fn app_preferences_defaults_deserialize_from_legacy_json() {
        let legacy = r#"{"translator":"OpenAI","target_language":"japanese"}"#;
        let prefs: AppPreferences = serde_json::from_str(legacy).unwrap();
        assert_eq!(prefs.translator, "OpenAI");
        assert_eq!(prefs.target_language, "japanese");
        assert_eq!(prefs.ocr_language, "Chinese and English");
        assert!(prefs.auto_translate_after_ocr);
        assert!(prefs.direct_target_translation);
        assert!(prefs.streaming_enabled);
        assert!(prefs.use_keychain);
        assert!(prefs.always_on_top);
        assert!(!prefs.vision_ocr);
        assert!(!prefs.translation_overlay);
        assert!(!prefs.clipboard_watch);
        assert_eq!(prefs.theme, ThemeChoice::System);
    }

    #[test]
    fn preferences_roundtrip_and_corrupt_file_falls_back() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("app_settings.json");
        let prefs = AppPreferences {
            translator: "Claude".into(),
            theme: ThemeChoice::Light,
            clipboard_watch: true,
            ..Default::default()
        };
        save_preferences_to(&path, &prefs);
        assert_eq!(load_preferences_from(&path), prefs);

        fs::write(&path, "garbage").unwrap();
        assert_eq!(load_preferences_from(&path), AppPreferences::default());
    }

    #[test]
    fn prompts_include_glossary_and_target_language() {
        let g =
            "\n\nUse these exact glossary translations when the terms appear:\n- \"a\" => \"b\"";
        let en = build_english_system_prompt(g);
        assert!(en.contains("natural, accurate English"));
        assert!(en.ends_with(g));
        let bi = build_bilingual_system_prompt("thai", "");
        assert!(bi.contains("natural thai"));
        assert!(bi.contains("\"english\""));
        assert!(bi.contains("\"target\""));
    }

    #[test]
    fn extracts_error_messages_from_provider_bodies() {
        assert_eq!(
            extract_api_error_message(r#"{"error":{"message":"Incorrect API key provided"}}"#)
                .as_deref(),
            Some("Incorrect API key provided")
        );
        assert_eq!(
            extract_api_error_message(
                r#"{"type":"error","error":{"type":"x","message":"overloaded"}}"#
            )
            .as_deref(),
            Some("overloaded")
        );
        assert_eq!(
            extract_api_error_message(r#"{"error":"plain string"}"#).as_deref(),
            Some("plain string")
        );
        assert!(extract_api_error_message("<html>502</html>").is_none());
        assert!(extract_api_error_message(r#"{"error":{"message":"  "}}"#).is_none());
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(500));
        let msg = extract_api_error_message(&long).unwrap();
        assert_eq!(msg.chars().count(), 201);
        assert!(msg.ends_with('…'));
    }

    #[test]
    fn friendly_errors_are_specific_per_status() {
        let err = |status, body: &str| ProviderError::Api {
            status,
            body: body.to_string(),
        };
        let m = friendly_error("OpenAI", &err(401, r#"{"error":{"message":"bad key"}}"#));
        assert!(m.contains("rejected the API key"));
        assert!(m.contains("bad key"));
        assert!(friendly_error("OpenAI", &err(403, "")).contains("rejected the API key"));
        assert!(friendly_error("Claude", &err(404, "")).contains("model name"));
        assert!(friendly_error("Gemini", &err(429, "")).contains("rate limit"));
        assert!(friendly_error("DeepSeek", &err(503, "")).contains("temporarily unavailable"));
        assert!(friendly_error("DeepSeek", &err(400, "")).contains("HTTP 400"));
        assert!(
            friendly_error("OpenAI", &ProviderError::MissingApiKey("OpenAI"))
                .contains("Missing OpenAI API key")
        );
        assert!(friendly_error("OpenAI", &ProviderError::EmptyResponse).contains("empty response"));
    }

    #[tokio::test]
    async fn connection_failures_get_provider_specific_hints() {
        // Port 1 is never listening, so this fails fast with a connect error.
        let err = translate_openai_compatible("http://127.0.0.1:1/v1", None, "m", "s", "t")
            .await
            .unwrap_err();
        assert!(friendly_error("Ollama (Local)", &err).contains("ollama serve"));
        assert!(friendly_error("OpenAI", &err).contains("internet connection"));
    }

    #[tokio::test]
    async fn openai_compatible_sends_auth_model_and_prompts() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk-test"))
            .and(body_partial_json(serde_json::json!({
                "model": "gpt-test",
                "messages": [
                    { "role": "system", "content": "SYS" },
                    { "role": "user", "content": "สวัสดี" }
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "content": "Hello" } }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let out = translate_openai_compatible(
            &format!("{}/v1/", server.uri()),
            Some(" sk-test "),
            "gpt-test",
            "SYS",
            "สวัสดี",
        )
        .await
        .unwrap();
        assert_eq!(out, "Hello");
    }

    #[tokio::test]
    async fn openai_compatible_without_key_sends_no_auth_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "content": "ok" } }]
            })))
            .mount(&server)
            .await;

        translate_openai_compatible(&format!("{}/v1", server.uri()), None, "m", "s", "t")
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].headers.contains_key("authorization"));
    }

    #[tokio::test]
    async fn openai_compatible_maps_http_errors_and_empty_bodies() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": {"message": "Invalid key"}})),
            )
            .mount(&server)
            .await;
        let err =
            translate_openai_compatible(&format!("{}/v1", server.uri()), Some("k"), "m", "s", "t")
                .await
                .unwrap_err();
        assert!(matches!(err, ProviderError::Api { status: 401, .. }));
        assert!(friendly_error("OpenAI", &err).contains("Invalid key"));

        let empty = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"choices": []})),
            )
            .mount(&empty)
            .await;
        let err =
            translate_openai_compatible(&format!("{}/v1", empty.uri()), Some("k"), "m", "s", "t")
                .await
                .unwrap_err();
        assert!(matches!(err, ProviderError::EmptyResponse));
    }

    #[tokio::test]
    async fn claude_sends_required_headers_and_parses_text_block() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "sk-ant"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(body_partial_json(serde_json::json!({
                "model": "claude-test", "system": "SYS"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{ "type": "text", "text": "Bonjour" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let out = translate_claude_at(&server.uri(), "sk-ant", "claude-test", "SYS", "Hello")
            .await
            .unwrap();
        assert_eq!(out, "Bonjour");

        let missing = translate_claude_at(&server.uri(), "  ", "m", "s", "t").await;
        assert!(matches!(
            missing,
            Err(ProviderError::MissingApiKey("Claude"))
        ));
    }

    #[tokio::test]
    async fn gemini_uses_key_query_and_parses_candidate_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path_regex(r"^/v1beta/models/gemini-test:generateContent$"))
            .and(query_param("key", "AIza"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "candidates": [{ "content": { "parts": [{ "text": "Hallo" }] } }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let out = translate_gemini_prompt_at(&server.uri(), "AIza", "gemini-test", "prompt")
            .await
            .unwrap();
        assert_eq!(out, "Hallo");

        let missing = translate_gemini_prompt_at(&server.uri(), "", "m", "p").await;
        assert!(matches!(
            missing,
            Err(ProviderError::MissingApiKey("Gemini"))
        ));
    }

    #[test]
    fn endpoints_all_at_points_every_service_at_the_mock() {
        let e = Endpoints::all_at("http://127.0.0.1:9/");
        assert_eq!(e.openai, "http://127.0.0.1:9/v1");
        assert_eq!(e.anthropic, "http://127.0.0.1:9");
        assert_eq!(e.google_translate, "http://127.0.0.1:9/translate_a/single");
        assert_eq!(Endpoints::default().openai, "https://api.openai.com/v1");
    }
}

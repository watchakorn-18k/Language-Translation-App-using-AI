use crate::apikey::ApikeyGpt;
use crate::gemini::GeminiApi;
use crate::google_translate::{GoogleTranslator, lang_code_for_name};
use crate::json_extract::trim_whitespace;
use crate::providers::{
    DEFAULT_CLAUDE_CONFIG_PATH, DEFAULT_CLAUDE_MODEL, DEFAULT_CUSTOM_BASE_URL,
    DEFAULT_CUSTOM_CONFIG_PATH, DEFAULT_CUSTOM_MODEL, DEFAULT_DEEPSEEK_CONFIG_PATH,
    DEFAULT_DEEPSEEK_MODEL, DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_CONFIG_PATH,
    DEFAULT_OLLAMA_MODEL, ProviderSettings, build_bilingual_system_prompt,
    build_english_system_prompt, load_app_preferences, load_gemini_settings, load_openai_settings,
    load_provider_settings, parse_bilingual_response, save_gemini_settings, save_openai_settings,
    save_provider_settings, translate_claude, translate_gemini_prompt, translate_openai_compatible,
};
use crate::streaming::{stream_claude, stream_gemini, stream_openai_compatible};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslatorProvider {
    GoogleTranslate,
    OpenAi,
    Gemini,
    Claude,
    DeepSeek,
    Ollama,
    CustomOpenAi,
}

impl TranslatorProvider {
    pub const ALL: &'static [TranslatorProvider] = &[
        TranslatorProvider::GoogleTranslate,
        TranslatorProvider::OpenAi,
        TranslatorProvider::Gemini,
        TranslatorProvider::Claude,
        TranslatorProvider::DeepSeek,
        TranslatorProvider::Ollama,
        TranslatorProvider::CustomOpenAi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TranslatorProvider::GoogleTranslate => "Google Translate",
            TranslatorProvider::OpenAi => "OpenAI",
            TranslatorProvider::Gemini => "Gemini",
            TranslatorProvider::Claude => "Claude",
            TranslatorProvider::DeepSeek => "DeepSeek",
            TranslatorProvider::Ollama => "Ollama (Local)",
            TranslatorProvider::CustomOpenAi => "Custom (OpenAI API)",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "OpenAI" | "ChatGPT" => TranslatorProvider::OpenAi,
            "Gemini" => TranslatorProvider::Gemini,
            "Claude" => TranslatorProvider::Claude,
            "DeepSeek" => TranslatorProvider::DeepSeek,
            "Ollama (Local)" => TranslatorProvider::Ollama,
            "Custom (OpenAI API)" => TranslatorProvider::CustomOpenAi,
            _ => TranslatorProvider::GoogleTranslate,
        }
    }

    pub fn needs_api_key(self) -> bool {
        !matches!(
            self,
            TranslatorProvider::GoogleTranslate | TranslatorProvider::Ollama
        )
    }

    pub fn needs_model(self) -> bool {
        !matches!(self, TranslatorProvider::GoogleTranslate)
    }

    pub fn needs_base_url(self) -> bool {
        matches!(
            self,
            TranslatorProvider::Ollama | TranslatorProvider::CustomOpenAi
        )
    }

    pub fn supports_vision(self) -> bool {
        matches!(
            self,
            TranslatorProvider::OpenAi
                | TranslatorProvider::Gemini
                | TranslatorProvider::Claude
                | TranslatorProvider::Ollama
                | TranslatorProvider::CustomOpenAi
        )
    }

    pub fn recommended_models_hint(self) -> &'static str {
        match self {
            TranslatorProvider::GoogleTranslate => "",
            TranslatorProvider::OpenAi => "gpt-6.2-sol · gpt-6.2-astra · gpt-6-luna",
            TranslatorProvider::Gemini => {
                "gemini-4-neon · gemini-4-hydrogen · gemini-4-argon · gemini-4-carbon"
            }
            TranslatorProvider::Claude => "claude-sonnet-5-5 · claude-opus-5-5 · claude-haiku-5-5",
            TranslatorProvider::DeepSeek => "deepseek-chat · deepseek-reasoner",
            TranslatorProvider::Ollama => "gemma4 · qwen3.5:9b · qwen3.5:4b",
            TranslatorProvider::CustomOpenAi => "Any model ID supported by your endpoint",
        }
    }

    pub fn load_settings(self) -> ProviderSettings {
        let use_keychain = load_app_preferences().use_keychain;
        match self {
            TranslatorProvider::GoogleTranslate => ProviderSettings {
                api_key: String::new(),
                model: String::new(),
                base_url: String::new(),
            },
            TranslatorProvider::OpenAi => load_openai_settings(use_keychain),
            TranslatorProvider::Gemini => load_gemini_settings(use_keychain),
            TranslatorProvider::Claude => load_provider_settings(
                DEFAULT_CLAUDE_CONFIG_PATH,
                "claude",
                DEFAULT_CLAUDE_MODEL,
                "",
                use_keychain,
            ),
            TranslatorProvider::DeepSeek => load_provider_settings(
                DEFAULT_DEEPSEEK_CONFIG_PATH,
                "deepseek",
                DEFAULT_DEEPSEEK_MODEL,
                "https://api.deepseek.com/v1",
                use_keychain,
            ),
            TranslatorProvider::Ollama => load_provider_settings(
                DEFAULT_OLLAMA_CONFIG_PATH,
                "ollama",
                DEFAULT_OLLAMA_MODEL,
                DEFAULT_OLLAMA_BASE_URL,
                false,
            ),
            TranslatorProvider::CustomOpenAi => load_provider_settings(
                DEFAULT_CUSTOM_CONFIG_PATH,
                "custom_openai",
                DEFAULT_CUSTOM_MODEL,
                DEFAULT_CUSTOM_BASE_URL,
                use_keychain,
            ),
        }
    }

    pub fn save_settings(self, api_key: &str, model: &str, base_url: &str, use_keychain: bool) {
        match self {
            TranslatorProvider::GoogleTranslate => {}
            TranslatorProvider::OpenAi => {
                let _ = save_openai_settings(api_key, model, use_keychain);
            }
            TranslatorProvider::Gemini => {
                let _ = save_gemini_settings(api_key, model, use_keychain);
            }
            TranslatorProvider::Claude => {
                let _ = save_provider_settings(
                    DEFAULT_CLAUDE_CONFIG_PATH,
                    "claude",
                    &ProviderSettings {
                        api_key: api_key.trim().to_string(),
                        model: if model.trim().is_empty() {
                            DEFAULT_CLAUDE_MODEL.to_string()
                        } else {
                            model.trim().to_string()
                        },
                        base_url: String::new(),
                    },
                    use_keychain,
                );
            }
            TranslatorProvider::DeepSeek => {
                let _ = save_provider_settings(
                    DEFAULT_DEEPSEEK_CONFIG_PATH,
                    "deepseek",
                    &ProviderSettings {
                        api_key: api_key.trim().to_string(),
                        model: if model.trim().is_empty() {
                            DEFAULT_DEEPSEEK_MODEL.to_string()
                        } else {
                            model.trim().to_string()
                        },
                        base_url: "https://api.deepseek.com/v1".to_string(),
                    },
                    use_keychain,
                );
            }
            TranslatorProvider::Ollama => {
                let _ = save_provider_settings(
                    DEFAULT_OLLAMA_CONFIG_PATH,
                    "ollama",
                    &ProviderSettings {
                        api_key: String::new(),
                        model: if model.trim().is_empty() {
                            DEFAULT_OLLAMA_MODEL.to_string()
                        } else {
                            model.trim().to_string()
                        },
                        base_url: if base_url.trim().is_empty() {
                            DEFAULT_OLLAMA_BASE_URL.to_string()
                        } else {
                            base_url.trim().to_string()
                        },
                    },
                    false,
                );
            }
            TranslatorProvider::CustomOpenAi => {
                let _ = save_provider_settings(
                    DEFAULT_CUSTOM_CONFIG_PATH,
                    "custom_openai",
                    &ProviderSettings {
                        api_key: api_key.trim().to_string(),
                        model: if model.trim().is_empty() {
                            DEFAULT_CUSTOM_MODEL.to_string()
                        } else {
                            model.trim().to_string()
                        },
                        base_url: if base_url.trim().is_empty() {
                            DEFAULT_CUSTOM_BASE_URL.to_string()
                        } else {
                            base_url.trim().to_string()
                        },
                    },
                    use_keychain,
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranslationOutcome {
    pub primary_english: String,
    pub secondary_localized: String,
}

#[derive(Debug, Clone, Copy)]
pub struct TranslateOptions {
    pub direct_target_translation: bool,
    pub streaming_enabled: bool,
}

impl Default for TranslateOptions {
    fn default() -> Self {
        Self {
            direct_target_translation: true,
            streaming_enabled: true,
        }
    }
}

/// Ensures default JSON config files exist at startup.
pub fn init_config_files() {
    let _ = ApikeyGpt::default().ensure_or_get_api_key();
    let _ = GeminiApi::default().check_has_json();
}

/// Full translation pipeline supporting:
/// - Google Translate (2-hop)
/// - Single-Hop Direct AI Translation (EN + target language in 1 API call with glossary)
/// - Streaming SSE chunks sent via `stream_tx` when `options.streaming_enabled` is true
pub async fn translate_text_with_options(
    input: &str,
    provider: TranslatorProvider,
    secondary_lang_name: &str,
    options: TranslateOptions,
    stream_tx: Option<UnboundedSender<String>>,
) -> TranslationOutcome {
    let google = GoogleTranslator::new();
    let dest_code = lang_code_for_name(secondary_lang_name).unwrap_or("th");

    if input.trim().is_empty() {
        let primary = "Fill in the text box".to_string();
        let secondary = google
            .translate(&primary, "en", dest_code)
            .await
            .unwrap_or_else(|_| primary.clone());
        return TranslationOutcome {
            primary_english: primary,
            secondary_localized: secondary,
        };
    }

    if provider == TranslatorProvider::GoogleTranslate {
        let primary = google
            .translate(input, "auto", "en")
            .await
            .unwrap_or_else(|e| format!("Google Translate error: {e}"));
        let primary = trim_whitespace(&primary);
        let secondary = google
            .translate(&primary, "en", dest_code)
            .await
            .unwrap_or_else(|_| primary.clone());
        return TranslationOutcome {
            primary_english: primary,
            secondary_localized: secondary,
        };
    }

    let cfg = provider.load_settings();
    if provider.needs_api_key() && cfg.api_key.trim().is_empty() {
        return TranslationOutcome {
            primary_english: format!("Missing {} API key", provider.label()),
            secondary_localized: format!("Set your {} API key in Settings", provider.label()),
        };
    }

    let system_prompt = if options.direct_target_translation {
        build_bilingual_system_prompt(secondary_lang_name)
    } else {
        build_english_system_prompt()
    };

    let raw_result = if options.streaming_enabled {
        if let Some(tx) = stream_tx {
            call_provider_streaming(provider, &cfg, &system_prompt, input, &tx).await
        } else {
            call_provider_once(provider, &cfg, &system_prompt, input).await
        }
    } else {
        call_provider_once(provider, &cfg, &system_prompt, input).await
    };

    let raw_response = match raw_result {
        Ok(text) => text,
        Err(e) => {
            return TranslationOutcome {
                primary_english: format!("{} error: {e}", provider.label()),
                secondary_localized: format!(
                    "Check your {} API key and model in Settings",
                    provider.label()
                ),
            };
        }
    };

    if options.direct_target_translation {
        if let Some((english, target)) = parse_bilingual_response(&raw_response) {
            return TranslationOutcome {
                primary_english: english,
                secondary_localized: target,
            };
        }
    }

    let primary = trim_whitespace(&raw_response);
    let secondary = google
        .translate(&primary, "en", dest_code)
        .await
        .unwrap_or_else(|_| primary.clone());

    TranslationOutcome {
        primary_english: primary,
        secondary_localized: secondary,
    }
}

async fn call_provider_once(
    provider: TranslatorProvider,
    cfg: &ProviderSettings,
    system_prompt: &str,
    input: &str,
) -> Result<String, crate::providers::ProviderError> {
    match provider {
        TranslatorProvider::OpenAi => {
            translate_openai_compatible(
                "https://api.openai.com/v1",
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
            )
            .await
        }
        TranslatorProvider::Gemini => {
            let full_prompt = format!("{system_prompt}\n\nInput text:\n{input}");
            translate_gemini_prompt(&cfg.api_key, &cfg.model, &full_prompt).await
        }
        TranslatorProvider::Claude => {
            translate_claude(&cfg.api_key, &cfg.model, system_prompt, input).await
        }
        TranslatorProvider::DeepSeek => {
            translate_openai_compatible(
                "https://api.deepseek.com/v1",
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
            )
            .await
        }
        TranslatorProvider::Ollama => {
            translate_openai_compatible(&cfg.base_url, None, &cfg.model, system_prompt, input).await
        }
        TranslatorProvider::CustomOpenAi => {
            translate_openai_compatible(
                &cfg.base_url,
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
            )
            .await
        }
        TranslatorProvider::GoogleTranslate => unreachable!(),
    }
}

async fn call_provider_streaming(
    provider: TranslatorProvider,
    cfg: &ProviderSettings,
    system_prompt: &str,
    input: &str,
    tx: &UnboundedSender<String>,
) -> Result<String, crate::providers::ProviderError> {
    match provider {
        TranslatorProvider::OpenAi => {
            stream_openai_compatible(
                "https://api.openai.com/v1",
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
                tx,
            )
            .await
        }
        TranslatorProvider::Gemini => {
            let full_prompt = format!("{system_prompt}\n\nInput text:\n{input}");
            stream_gemini(&cfg.api_key, &cfg.model, &full_prompt, tx).await
        }
        TranslatorProvider::Claude => {
            stream_claude(&cfg.api_key, &cfg.model, system_prompt, input, tx).await
        }
        TranslatorProvider::DeepSeek => {
            stream_openai_compatible(
                "https://api.deepseek.com/v1",
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
                tx,
            )
            .await
        }
        TranslatorProvider::Ollama => {
            stream_openai_compatible(&cfg.base_url, None, &cfg.model, system_prompt, input, tx)
                .await
        }
        TranslatorProvider::CustomOpenAi => {
            stream_openai_compatible(
                &cfg.base_url,
                Some(&cfg.api_key),
                &cfg.model,
                system_prompt,
                input,
                tx,
            )
            .await
        }
        TranslatorProvider::GoogleTranslate => unreachable!(),
    }
}

/// Convenience wrapper for non-streaming callers.
pub async fn translate_text(
    input: &str,
    provider: TranslatorProvider,
    secondary_lang_name: &str,
) -> TranslationOutcome {
    translate_text_with_options(
        input,
        provider,
        secondary_lang_name,
        TranslateOptions::default(),
        None,
    )
    .await
}

/// Re-translate the existing English result into a newly chosen secondary language.
pub async fn retranslate_secondary(primary_english: &str, secondary_lang_name: &str) -> String {
    if primary_english.trim().is_empty() {
        return String::new();
    }
    let google = GoogleTranslator::new();
    let dest_code = lang_code_for_name(secondary_lang_name).unwrap_or("th");
    google
        .translate(primary_english, "en", dest_code)
        .await
        .unwrap_or_else(|_| primary_english.to_string())
}

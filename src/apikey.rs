use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

pub const DEFAULT_CHATGPT_CONFIG_PATH: &str = "api_key_chatgpt.json";
/// Value written by the legacy Python app; treated as "no key".
pub const DEFAULT_CHATGPT_API_KEY_PLACEHOLDER: &str = "API key for the CHATGPT";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatGptConfig {
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

/// Maps the legacy placeholder to an empty key and trims whitespace.
pub fn clean_api_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == DEFAULT_CHATGPT_API_KEY_PLACEHOLDER {
        String::new()
    } else {
        trimmed.to_string()
    }
}

pub fn read_config(path: &Path) -> Option<ChatGptConfig> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_config(path: &Path, cfg: &ChatGptConfig) -> io::Result<()> {
    let json = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn legacy_placeholder_is_treated_as_missing_key() {
        assert_eq!(clean_api_key(DEFAULT_CHATGPT_API_KEY_PLACEHOLDER), "");
        assert_eq!(clean_api_key("  sk-123  "), "sk-123");
    }

    #[test]
    fn config_roundtrips_and_tolerates_missing_model() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_key_chatgpt.json");
        assert!(read_config(&path).is_none());

        let cfg = ChatGptConfig {
            api_key: "sk-abc".into(),
            model: "gpt-6.2-sol".into(),
        };
        write_config(&path, &cfg).unwrap();
        assert_eq!(read_config(&path), Some(cfg));

        fs::write(&path, r#"{"api_key":"only-key"}"#).unwrap();
        let loaded = read_config(&path).unwrap();
        assert_eq!(loaded.api_key, "only-key");
        assert_eq!(loaded.model, "");
    }
}

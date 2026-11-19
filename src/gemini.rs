use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

pub const DEFAULT_GEMINI_CONFIG_PATH: &str = "api_gemini.json";
/// Value written by the legacy Python app; treated as "no key".
pub const DEFAULT_GEMINI_API_KEY_PLACEHOLDER: &str =
    "get API key for the Gemini go to https://aistudio.google.com/app/apikey";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeminiConfig {
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

/// Maps the legacy placeholder to an empty key and trims whitespace.
pub fn clean_api_key(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == DEFAULT_GEMINI_API_KEY_PLACEHOLDER {
        String::new()
    } else {
        trimmed.to_string()
    }
}

pub fn read_config(path: &Path) -> Option<GeminiConfig> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_config(path: &Path, cfg: &GeminiConfig) -> io::Result<()> {
    let json = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn legacy_placeholder_is_treated_as_missing_key() {
        assert_eq!(clean_api_key(DEFAULT_GEMINI_API_KEY_PLACEHOLDER), "");
        assert_eq!(clean_api_key(" AIza-key "), "AIza-key");
    }

    #[test]
    fn config_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_gemini.json");
        let cfg = GeminiConfig {
            api_key: "AIza123".into(),
            model: "gemini-4-neon".into(),
        };
        write_config(&path, &cfg).unwrap();
        assert_eq!(read_config(&path), Some(cfg));
    }
}

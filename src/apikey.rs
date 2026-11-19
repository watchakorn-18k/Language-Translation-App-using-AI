use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_CHATGPT_CONFIG_PATH: &str = "api_key_chatgpt.json";
pub const DEFAULT_CHATGPT_API_KEY_PLACEHOLDER: &str = "API key for the CHATGPT";
pub const DEFAULT_CHATGPT_MODEL: &str = "gpt-6.2-sol";

#[derive(Debug, Error)]
pub enum ApikeyError {
    #[error("I/O error accessing `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Invalid JSON in `{path}`: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatGptConfig {
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    DEFAULT_CHATGPT_MODEL.to_string()
}

impl Default for ChatGptConfig {
    fn default() -> Self {
        Self {
            api_key: DEFAULT_CHATGPT_API_KEY_PLACEHOLDER.to_string(),
            model: DEFAULT_CHATGPT_MODEL.to_string(),
        }
    }
}

/// Port of `src/Apikey.py` (`ApikeyGPT`).
#[derive(Debug, Clone)]
pub struct ApikeyGpt {
    path: PathBuf,
    default_data: ChatGptConfig,
}

impl Default for ApikeyGpt {
    fn default() -> Self {
        Self::new(DEFAULT_CHATGPT_CONFIG_PATH)
    }
}

impl ApikeyGpt {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            default_data: ChatGptConfig::default(),
        }
    }

    pub fn create_file_json(&self) -> Result<(), ApikeyError> {
        let json = serde_json::to_string_pretty(&self.default_data).map_err(|source| {
            ApikeyError::Json {
                path: self.path.clone(),
                source,
            }
        })?;
        fs::write(&self.path, json).map_err(|source| ApikeyError::Io {
            path: self.path.clone(),
            source,
        })
    }

    /// Ensures `api_key_chatgpt.json` exists; creates it with defaults if missing,
    /// otherwise returns the stored API key.
    pub fn ensure_or_get_api_key(&self) -> Result<Option<String>, ApikeyError> {
        if !self.path.exists() {
            self.create_file_json()?;
            Ok(None)
        } else {
            Ok(Some(self.get_api_key()?))
        }
    }

    pub fn load_config(&self) -> Result<ChatGptConfig, ApikeyError> {
        let content = fs::read_to_string(&self.path).map_err(|source| ApikeyError::Io {
            path: self.path.clone(),
            source,
        })?;
        serde_json::from_str(&content).map_err(|source| ApikeyError::Json {
            path: self.path.clone(),
            source,
        })
    }

    pub fn get_api_key(&self) -> Result<String, ApikeyError> {
        Ok(self.load_config()?.api_key)
    }

    pub fn get_model(&self) -> Result<String, ApikeyError> {
        let model = self.load_config()?.model;
        if model.trim().is_empty() {
            Ok(DEFAULT_CHATGPT_MODEL.to_string())
        } else {
            Ok(model)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_and_reads_default_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_key_chatgpt.json");
        let mgr = ApikeyGpt::new(&path);

        assert_eq!(mgr.ensure_or_get_api_key().unwrap(), None);
        assert!(path.exists());
        assert_eq!(
            mgr.get_api_key().unwrap(),
            DEFAULT_CHATGPT_API_KEY_PLACEHOLDER
        );
        assert_eq!(mgr.get_model().unwrap(), DEFAULT_CHATGPT_MODEL);
    }
}

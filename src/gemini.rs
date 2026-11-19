use crate::json_extract::extract_translation_result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const DEFAULT_GEMINI_CONFIG_PATH: &str = "api_gemini.json";
pub const DEFAULT_GEMINI_API_KEY_PLACEHOLDER: &str =
    "get API key for the Gemini go to https://aistudio.google.com/app/apikey";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-4-neon";

#[derive(Debug, Error)]
pub enum GeminiError {
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
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Gemini API returned status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("Gemini response did not contain text")]
    EmptyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeminiConfig {
    pub api_key: String,
    #[serde(default = "default_gemini_model")]
    pub model: String,
}

fn default_gemini_model() -> String {
    DEFAULT_GEMINI_MODEL.to_string()
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: DEFAULT_GEMINI_API_KEY_PLACEHOLDER.to_string(),
            model: DEFAULT_GEMINI_MODEL.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<Content>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    #[serde(default)]
    parts: Vec<CandidatePart>,
}

#[derive(Debug, Deserialize)]
struct CandidatePart {
    text: Option<String>,
}

/// Port of `src/Gemini.py` (`GeminiAPI`).
#[derive(Debug, Clone)]
pub struct GeminiApi {
    path: PathBuf,
    client: reqwest::Client,
}

impl Default for GeminiApi {
    fn default() -> Self {
        Self::new(DEFAULT_GEMINI_CONFIG_PATH)
    }
}

impl GeminiApi {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            client: reqwest::Client::new(),
        }
    }

    pub fn create_file_json(&self) -> Result<(), GeminiError> {
        let json = serde_json::to_string_pretty(&GeminiConfig::default()).map_err(|source| {
            GeminiError::Json {
                path: self.path.clone(),
                source,
            }
        })?;
        fs::write(&self.path, json).map_err(|source| GeminiError::Io {
            path: self.path.clone(),
            source,
        })
    }

    pub fn check_has_json(&self) -> Result<(), GeminiError> {
        if !self.path.exists() {
            self.create_file_json()?;
        }
        Ok(())
    }

    pub async fn prompt_gemini_run(&self, prompt: &str) -> Result<String, GeminiError> {
        self.check_has_json()?;
        let content = fs::read_to_string(&self.path).map_err(|source| GeminiError::Io {
            path: self.path.clone(),
            source,
        })?;
        let mut config: GeminiConfig =
            serde_json::from_str(&content).map_err(|source| GeminiError::Json {
                path: self.path.clone(),
                source,
            })?;

        if config.api_key == DEFAULT_GEMINI_API_KEY_PLACEHOLDER || config.api_key.trim().is_empty()
        {
            return Ok(
                "Please add the gemini key to the `api_gemini.json` file first.".to_string(),
            );
        }

        if config.model.trim().is_empty() {
            config.model = DEFAULT_GEMINI_MODEL.to_string();
        }

        let full_prompt = format!(
            "You are a translation expert. Translate into the target language while preserving the original sentence structure and meaning exactly.Translate in a neutral way, . try to preserve unique name such as company's name or brand's name , this is the prompt:'{prompt}' translate to english only answer in json format is{{\"translation_result\": \"<result>\"}}"
        );

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            config.model
        );

        let body = GenerateContentRequest {
            contents: vec![Content {
                parts: vec![Part { text: full_prompt }],
            }],
        };

        let resp = self
            .client
            .post(&url)
            .query(&[("key", config.api_key.as_str())])
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(GeminiError::Api {
                status: status.as_u16(),
                body: text,
            });
        }

        let parsed: GenerateContentResponse = resp.json().await?;
        let text = parsed
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content)
            .and_then(|c| c.parts.into_iter().find_map(|p| p.text))
            .ok_or(GeminiError::EmptyResponse)?;

        Ok(extract_translation_result(&text).unwrap_or(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn placeholder_returns_prompt_message() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("api_gemini.json");
        let api = GeminiApi::new(&path);

        let out = api.prompt_gemini_run("สวัสดี").await.unwrap();
        assert_eq!(
            out,
            "Please add the gemini key to the `api_gemini.json` file first."
        );
        assert!(path.exists());
    }
}

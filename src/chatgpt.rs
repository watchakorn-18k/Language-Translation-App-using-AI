use serde::{Deserialize, Serialize};
use thiserror::Error;

const OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";
const SYSTEM_PROMPT: &str = "You are a translation expert. Translate into the target language while preserving the original sentence structure and meaning exactly.Translate in a neutral way, . try to preserve unique name such as company's name or brand's name.";

#[derive(Debug, Error)]
pub enum ChatGptError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("OpenAI API returned status {status}: {body}")]
    Api { status: u16, body: String },
    #[error("OpenAI response did not contain any choices")]
    EmptyResponse,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

/// Port of `src/ChatGPT.py` (`ChatGPTAPI`).
#[derive(Debug, Clone)]
pub struct ChatGptApi {
    api_key: String,
    client: reqwest::Client,
    endpoint: String,
}

impl ChatGptApi {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: reqwest::Client::new(),
            endpoint: OPENAI_CHAT_COMPLETIONS_URL.to_string(),
        }
    }

    pub async fn generate_response(
        &self,
        prompt: &str,
        model_engine: Option<&str>,
    ) -> Result<String, ChatGptError> {
        let model = model_engine
            .filter(|m| !m.trim().is_empty())
            .unwrap_or("gpt-6.2-sol");

        let req = ChatCompletionRequest {
            model,
            temperature: 0.1,
            max_tokens: 2000,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: format!("original_text:{prompt}\n Your translation in english:"),
                },
            ],
        };

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ChatGptError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: ChatCompletionResponse = resp.json().await?;
        parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or(ChatGptError::EmptyResponse)
    }
}

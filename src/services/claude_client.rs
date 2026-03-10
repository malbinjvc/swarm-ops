use async_trait::async_trait;
use crate::models::{AgentError, ClaudeMessage, ClaudeRequest, ClaudeResponse};

/// Trait for the Claude API client — enables mocking in tests.
#[async_trait]
pub trait ClaudeClient: Send + Sync {
    async fn send_message(&self, system_prompt: &str, user_message: &str)
        -> Result<String, AgentError>;
}

/// Real implementation that calls the Anthropic Messages API.
pub struct HttpClaudeClient {
    api_key: String,
    client: reqwest::Client,
    model: String,
}

impl HttpClaudeClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl ClaudeClient for HttpClaudeClient {
    async fn send_message(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<String, AgentError> {
        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![
                ClaudeMessage {
                    role: "user".to_string(),
                    content: format!("{}\n\n{}", system_prompt, user_message),
                },
            ],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::ClaudeApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(AgentError::ClaudeApiError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let claude_response: ClaudeResponse = response
            .json()
            .await
            .map_err(|e| AgentError::ClaudeApiError(e.to_string()))?;

        Ok(claude_response.text())
    }
}

/// Mock implementation for tests — returns deterministic responses.
#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct MockClaudeClient {
        pub response: String,
        pub should_fail: bool,
    }

    impl MockClaudeClient {
        pub fn new(response: impl Into<String>) -> Self {
            Self {
                response: response.into(),
                should_fail: false,
            }
        }

        pub fn failing() -> Self {
            Self {
                response: String::new(),
                should_fail: true,
            }
        }
    }

    #[async_trait]
    impl ClaudeClient for MockClaudeClient {
        async fn send_message(
            &self,
            _system_prompt: &str,
            _user_message: &str,
        ) -> Result<String, AgentError> {
            if self.should_fail {
                Err(AgentError::ClaudeApiError("Mock failure".to_string()))
            } else {
                Ok(self.response.clone())
            }
        }
    }
}

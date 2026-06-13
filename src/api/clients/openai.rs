//! The OpenAI API client.

use crate::{
    api::clients::openai_compatible::OpenAICompatible,
    config::{Config, OpenAIConfig},
    models::{CompletionRequest, CompletionResponse},
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct OpenAIClient {
    config: OpenAIConfig,
}

impl OpenAICompatible for OpenAIClient {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        if self.config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }
        self.request_completion(request).await
    }

    fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.config.api_key)
    }
}

impl ErgonClient for OpenAIClient {
    async fn complete_message(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse> {
        log::info!(
            "OpenAIClient: Completing message with {} messages using model {}",
            request.messages.len(),
            request.model
        );
        if request.messages.is_empty() {
            Err(anyhow::anyhow!("No messages provided".to_string()))
        } else {
            self.request(request).await
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<Model>> {
        OpenAICompatible::list_models(self).await
    }
}

impl Default for OpenAIClient {
    fn default() -> Self {
        OpenAIClient {
            config: Config::default().openai,
        }
    }
}

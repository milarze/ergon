//! vLLM API Client

use crate::{
    api::clients::openai_compatible::OpenAICompatible,
    config::{Config, VllmConfig},
    models::{CompletionRequest, CompletionResponse},
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct VllmClient {
    config: VllmConfig,
}

impl OpenAICompatible for VllmClient {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        self.request_completion(request).await
    }

    fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn api_key(&self) -> Option<&str> {
        None
    }
}

impl ErgonClient for VllmClient {
    async fn complete_message(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse> {
        if request.messages.is_empty() {
            return Err(anyhow::anyhow!("No messages provided".to_string()));
        }
        self.request(request).await
    }

    async fn list_models(&self) -> anyhow::Result<Vec<Model>> {
        if self.config.model.is_empty() {
            return Err(anyhow::anyhow!("vLLM model is not configured".to_string()));
        }
        Ok(vec![Model {
            name: self.config.model.clone(),
            id: self.config.model.clone(),
        }])
    }
}

impl Default for VllmClient {
    fn default() -> Self {
        let config = Config::default().vllm;
        Self { config }
    }
}

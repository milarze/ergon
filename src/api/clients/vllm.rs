//! vLLM API Client

use crate::{
    config::{Config, VllmConfig},
    models::{CompletionRequest, CompletionResponse},
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct VllmClient {
    config: VllmConfig,
}

impl VllmClient {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            log::error!("OpenAIClient: Request failed with error: {}", error_text);
            return Err(anyhow::anyhow!("Error: {}", error_text));
        }
        let text_data = response.text().await?;
        log::info!("vLLMClient: Response data: {}", text_data);
        let completion_response: CompletionResponse = self.deserialize_response(&text_data)?;
        Ok(completion_response)
    }

    fn deserialize_response(&self, response_text: &str) -> anyhow::Result<CompletionResponse> {
        let completion_response: CompletionResponse = serde_json::from_str(response_text)
            .map_err(anyhow::Error::from)
            .unwrap();
        Ok(completion_response)
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

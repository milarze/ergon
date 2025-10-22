//! The OpenAI API client.

use crate::{
    config::{Config, OpenAIConfig},
    models::{CompletionRequest, CompletionResponse},
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct OpenAIClient {
    config: OpenAIConfig,
}

impl OpenAIClient {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        if self.config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }
        let client = reqwest::Client::new();
        let url = format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
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
        let completion_response: CompletionResponse = serde_json::from_str(&text_data)
            .map_err(anyhow::Error::from)
            .unwrap();
        Ok(completion_response)
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
        log::info!("OpenAIClient: Fetching available models");
        if self.config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }

        let client = reqwest::Client::new();
        let url = format!("{}/models", self.config.endpoint.trim_end_matches('/'));

        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await.map_err(anyhow::Error::from)?;
                    let models = json["data"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|model| model["id"].as_str())
                        .filter(|id| id.contains("gpt"))
                        .map(|s| Model {
                            name: s.to_string(),
                            id: s.to_string(),
                        })
                        .collect();
                    Ok(models)
                } else {
                    log::error!(
                        "OpenAIClient: List models failed with status: {}",
                        resp.status()
                    );
                    Err(anyhow::anyhow!("Error: {}", resp.status()))
                }
            }
            Err(e) => {
                log::error!("OpenAIClient: List models request failed: {}", e);
                Err(anyhow::anyhow!("Request failed: {}", e))
            }
        }
    }
}

impl Default for OpenAIClient {
    fn default() -> Self {
        OpenAIClient {
            config: Config::default().openai,
        }
    }
}

//! The Claude API client.

use crate::{
    config::{AnthropicConfig, Config},
    models::{Choice, CompletionRequest, CompletionResponse, Message},
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
}

impl AnthropicClient {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse> {
        if self.config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }
        let client = reqwest::Client::new();
        let url = format!("{}/messages", self.config.endpoint.trim_end_matches('/'));
        let data = self.serialize_request(request)?;
        let response = client
            .post(url)
            .header("x-api-key", self.config.api_key.clone())
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            log::error!("OpenAIClient: Request failed with error: {}", error_text);
            return Err(anyhow::anyhow!("Error: {}", error_text));
        }
        log::info!(
            "AnthropicClient: Request successful with status: {}",
            response.status()
        );
        let text_data = response.text().await?;
        log::info!("AnthropicClient: Response data: {}", text_data);
        let completion_response: CompletionResponse = self.deserialize_response(text_data)?;
        Ok(completion_response)
    }

    async fn request_models(&self) -> anyhow::Result<Vec<Model>> {
        log::info!("AnthropicClient: Requesting available models");
        if self.config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }
        let client = reqwest::Client::new();
        let url = format!("{}/models", self.config.endpoint.trim_end_matches('/'));
        let response = client
            .get(url)
            .header("x-api-key", self.config.api_key.clone())
            .header("anthropic-version", "2023-06-01")
            .send()
            .await;
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await.map_err(anyhow::Error::from)?;
                    let models = json
                        .get("data")
                        .and_then(|m| m.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|model| {
                                    let id = model
                                        .get("id")
                                        .and_then(|n| n.as_str())
                                        .map(|s| s.to_string());
                                    let name = model
                                        .get("display_name")
                                        .and_then(|n| n.as_str())
                                        .map(|s| s.to_string());
                                    Some(Model {
                                        name: name?,
                                        id: id?,
                                    })
                                })
                                .collect::<Vec<Model>>()
                        })
                        .unwrap_or_default();
                    log::info!("AnthropicClient: Available models: {:?}", models);
                    Ok(models)
                } else {
                    let status = resp.status();
                    let body = resp.text().await.map_err(anyhow::Error::from)?;
                    log::error!("AnthropicClient: Request failed with status: {}", status);
                    log::error!("AnthropicClient: Response body: {:?}", body);
                    Err(anyhow::anyhow!("Error: {}", status))
                }
            }
            Err(e) => {
                log::error!("AnthropicClient: Request failed: {}", e);
                Err(anyhow::anyhow!("Request failed: {}", e))
            }
        }
    }

    fn serialize_request(&self, request: CompletionRequest) -> anyhow::Result<serde_json::Value> {
        let request_json = serde_json::json!(request);
        match request_json {
            serde_json::Value::Object(mut map) => {
                map.insert(
                    "max_tokens".to_string(),
                    serde_json::Value::Number(self.config.max_tokens.into()),
                );
                Ok(serde_json::Value::Object(map))
            }
            _ => Err(anyhow::anyhow!("Invalid request format")),
        }
    }

    fn deserialize_response(&self, response_text: String) -> anyhow::Result<CompletionResponse> {
        let parsed_json: serde_json::Value =
            serde_json::from_str(&response_text).map_err(anyhow::Error::from)?;
        Ok(CompletionResponse {
            id: parsed_json
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            object: parsed_json
                .get("object")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            created: parsed_json
                .get("created")
                .and_then(|v| v.as_u64())
                .unwrap_or_default(),
            model: parsed_json
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            choices: vec![Choice {
                index: 0,
                messages: parsed_json
                    .get("content")
                    .and_then(|v| self.deserialize_content(v).ok())
                    .unwrap_or_default(),
                finish_reason: parsed_json
                    .get("stop_reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }],
        })
    }

    fn deserialize_content(&self, content: &serde_json::Value) -> anyhow::Result<Vec<Message>> {
        if let serde_json::Value::Array(arr) = content {
            let messages = arr
                .into_iter()
                .map(|msg| {
                    Message::assistant(
                        msg.get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    )
                })
                .collect();
            Ok(messages)
        } else {
            Err(anyhow::anyhow!("Invalid content format"))
        }
    }
}

impl ErgonClient for AnthropicClient {
    async fn complete_message(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse> {
        log::info!(
            "AnthropicClient: Completing message with {} messages using model {}",
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
        log::info!("AnthropicClient: Listing models");
        self.request_models().await
    }
}

impl Default for AnthropicClient {
    fn default() -> Self {
        AnthropicClient {
            config: Config::default().anthropic,
        }
    }
}

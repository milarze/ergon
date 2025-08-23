//! The Claude API client.

use crate::{
    config::{AnthropicConfig, Config},
    ui::ChatMessage,
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    config: AnthropicConfig,
}

impl AnthropicClient {
    async fn request(&self, messages: Vec<ChatMessage>, model: &str) -> Result<String, String> {
        log::info!(
            "AnthropicClient: Requesting completion for {} messages with model {}",
            messages.len(),
            model
        );
        if self.config.api_key.is_empty() {
            return Err("API key is not set".to_string());
        }
        let client = reqwest::Client::new();
        let url = format!("{}/messages", self.config.endpoint.trim_end_matches('/'));
        let data = self.serialize_messages(messages, model)?;
        let response = client
            .post(url)
            .header("x-api-key", self.config.api_key.clone())
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                    let contents = json
                        .get("content")
                        .and_then(|c| c.as_array())
                        .map(|arr| arr.iter())
                        .into_iter()
                        .flatten()
                        .filter_map(|c| {
                            log::info!("AnthropicClient: Content item: {:?}", c);
                            // Only process items with type "text"
                            if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                                c["text"].as_str()
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<&str>>();
                    log::info!("AnthropicClient: Response content: {:?}", contents);
                    Ok(contents.join("\n"))
                } else {
                    let status = resp.status();
                    let body = resp.text().await.map_err(|e| e.to_string())?;
                    log::error!("AnthropicClient: Request failed with status: {}", status);
                    log::error!("AnthropicClient: Response body: {:?}", body);
                    Err(format!("Error: {}", status))
                }
            }
            Err(e) => {
                log::error!("AnthropicClient: Request failed: {}", e);
                Err(format!("Request failed: {}", e))
            }
        }
    }

    async fn request_models(&self) -> Result<Vec<Model>, String> {
        log::info!("AnthropicClient: Requesting available models");
        if self.config.api_key.is_empty() {
            return Err("API key is not set".to_string());
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
                    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
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
                    let body = resp.text().await.map_err(|e| e.to_string())?;
                    log::error!("AnthropicClient: Request failed with status: {}", status);
                    log::error!("AnthropicClient: Response body: {:?}", body);
                    Err(format!("Error: {}", status))
                }
            }
            Err(e) => {
                log::error!("AnthropicClient: Request failed: {}", e);
                Err(format!("Request failed: {}", e))
            }
        }
    }

    fn serialize_messages(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<serde_json::Value, String> {
        let messages: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|msg| {
                serde_json::json!({
                    "role": match msg.sender {
                        crate::ui::Sender::User => "user",
                        crate::ui::Sender::Bot => "assistant",
                    },
                    "content": serde_json::json!([{
                        "type": "text",
                        "text": msg.content,
                    }]),
                })
            })
            .collect();
        let serialized = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": self.config.max_tokens,
        });
        Ok(serialized)
    }
}

impl ErgonClient for AnthropicClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, String> {
        log::info!(
            "AnthropicClient: Completing message with {} messages using model {}",
            messages.len(),
            model
        );
        if messages.is_empty() {
            Err("No messages provided".to_string())
        } else {
            self.request(messages, model).await
        }
    }

    async fn list_models(&self) -> Result<Vec<Model>, String> {
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

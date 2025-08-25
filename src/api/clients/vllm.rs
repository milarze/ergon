//! vLLM API Client

use crate::{
    config::{Config, VllmConfig},
    ui::ChatMessage,
};

use super::{ErgonClient, Model};

#[derive(Debug, Clone)]
pub struct VllmClient {
    config: VllmConfig,
}

impl VllmClient {
    async fn request(&self, messages: Vec<ChatMessage>, model: &str) -> Result<String, String> {
        log::info!(
            "VllmClient: Requesting completion for {} messages with model {}",
            messages.len(),
            model
        );
        if self.config.endpoint.is_empty() {
            return Err("vLLM endpoint is not set".to_string());
        }
        let client = reqwest::Client::new();
        let url = format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        let data = self.serialize_messages(messages, model)?;
        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await;
        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
                    Ok(json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string())
                } else {
                    log::error!("VllmClient: Request failed with status: {}", resp.status());
                    Err(format!("Error: {}", resp.status()))
                }
            }
            Err(e) => {
                log::error!("VllmClient: Request failed: {}", e);
                Err(format!("Request failed: {}", e))
            }
        }
    }

    fn serialize_messages(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<serde_json::Value, String> {
        let msgs: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| {
                let role = match m.sender {
                    crate::ui::Sender::User => "user",
                    crate::ui::Sender::Bot => "assistant",
                };
                serde_json::json!({
                    "role": role,
                    "content": m.content,
                })
            })
            .collect();
        let data = serde_json::json!({
            "model": model,
            "messages": msgs,
        });
        Ok(data)
    }
}

impl ErgonClient for VllmClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, String> {
        if messages.is_empty() {
            return Err("No messages provided".to_string());
        }
        self.request(messages, model).await
    }

    async fn list_models(&self) -> Result<Vec<Model>, String> {
        if self.config.model.is_empty() {
            return Err("vLLM model is not configured".to_string());
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

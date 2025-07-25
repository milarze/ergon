//! The OpenAI API client.

use crate::{
    config::{Config, OpenAIConfig},
    ui::ChatMessage,
};

use super::{ErgonClient, Models};

#[derive(Debug, Clone)]
pub struct OpenAIClient {
    config: OpenAIConfig,
}

impl OpenAIClient {
    async fn request(&self, messages: Vec<ChatMessage>, model: Models) -> Result<String, String> {
        println!(
            "OpenAIClient: Requesting completion for {} messages with model {}",
            messages.len(),
            model
        );
        if self.config.api_key.is_empty() {
            return Err("API key is not set".to_string());
        }
        let client = reqwest::Client::new();
        let url = format!(
            "{}/chat/completions",
            self.config.endpoint.trim_end_matches('/')
        );
        let data = self.serialize_messages(messages, model)?;
        println!("OpenAIClient: Sending request to {}", url);
        println!("OpenAIClient: API Key: {}", self.config.api_key);
        println!("OpenAIClient: Request data: {}", data);
        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
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
                    println!(
                        "OpenAIClient: Request failed with status: {}",
                        resp.status()
                    );
                    Err(format!("Error: {}", resp.status()))
                }
            }
            Err(e) => {
                println!("OpenAIClient: Request failed: {}", e);
                Err(format!("Request failed: {}", e))
            }
        }
    }

    fn serialize_messages(
        &self,
        messages: Vec<ChatMessage>,
        model: Models,
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
            "model": model.to_string(),
            "messages": messages,
        });
        Ok(serialized)
    }
}

impl ErgonClient for OpenAIClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: Models,
    ) -> Result<String, String> {
        println!(
            "OpenAIClient: Completing message with {} messages",
            messages.len()
        );
        if messages.is_empty() {
            Err("No messages provided".to_string())
        } else {
            self.request(messages, model).await
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

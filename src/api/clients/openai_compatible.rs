use serde_json::json;

use crate::{api::clients::Model, models::{CompletionRequest, CompletionResponse, Content, Message}};

pub trait OpenAICompatible {
    async fn request(&self, request: CompletionRequest) -> anyhow::Result<CompletionResponse>;

    fn endpoint(&self) -> &str;

    fn api_key(&self) -> Option<&str>;

    async fn request_completion(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse> {
        let client = reqwest::Client::new();
        let url = format!("{}/chat/completions", self.endpoint().trim_end_matches('/'));

        let json_request = serde_json::json!({
            "model": request.model,
            "messages": request.messages.iter().map(OpenAIMessageAdapter::convert_message).collect::<Vec<_>>(),
            "temperature": request.temperature,
            "tools": request.tools,
        });

        log::info!("OpenAIClient: Sending request to {}", url);
        log::info!("OpenAIClient: Request payload: {}", json_request);
        let mut req = client.post(url);
        if let Some(api_key) = self.api_key() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }
        req = req.header("Content-Type", "application/json");
        req = req.json(&json_request);
        let response = req.send().await?;

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

    async fn list_models(&self) -> anyhow::Result<Vec<Model>> {
        log::info!("OpenAIClient: Fetching available models");
        if self.api_key().is_none() {
            return Err(anyhow::anyhow!("API key is not set".to_string()));
        }

        let client = reqwest::Client::new();
        let url = format!("{}/models", self.endpoint().trim_end_matches('/'));

        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.api_key().unwrap()))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json: serde_json::Value = resp.json().await.map_err(anyhow::Error::from)?;
                    let models: Vec<_> = json["data"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|model| model["id"].as_str())
                        .map(|s| Model {
                            name: s.to_string(),
                            id: s.to_string(),
                        })
                        .collect();
                    log::info!("OpenAIClient: Successfully fetched {} models", models.len());
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

struct OpenAIMessageAdapter;
impl OpenAIMessageAdapter {
    fn convert_message(msg: &Message) -> serde_json::Value {
        match msg.role.as_str() {
            "tool" => {
                // Extract content from Content::ToolResult and convert to string
                let content = msg
                    .content
                    .iter()
                    .find_map(|c| match c {
                        Content::ToolResult { content, .. } => Some(content.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                json!({
                    "role": "tool",
                    "content": content,
                    "tool_call_id": msg.tool_call_id
                })
            }
            _ => serde_json::to_value(msg).unwrap(),
        }
    }
}

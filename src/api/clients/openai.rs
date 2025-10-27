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

#[cfg(test)]
mod tests {
    use crate::models::{Content, Message, CompletionRequest};

    #[test]
    fn test_text_content_serialization() {
        let content = Content::text("Hello, GPT!");
        let json = serde_json::to_value(&content).unwrap();

        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello, GPT!");
    }

    #[test]
    fn test_text_content_deserialization() {
        let json = r#"{"type": "text", "text": "Hello from OpenAI!"}"#;
        let content: Content = serde_json::from_str(json).unwrap();

        match content {
            Content::Text { text } => assert_eq!(text, "Hello from OpenAI!"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_image_url_serialization() {
        let content = Content::image_url("https://example.com/image.jpg");
        let json = serde_json::to_value(&content).unwrap();

        assert_eq!(json["type"], "image_url");
        assert_eq!(json["image_url"]["url"], "https://example.com/image.jpg");
        assert!(json["image_url"].get("detail").is_none());
    }

    #[test]
    fn test_image_url_with_detail_serialization() {
        let content = Content::image_url_with_detail("https://example.com/image.jpg", "high");
        let json = serde_json::to_value(&content).unwrap();

        assert_eq!(json["type"], "image_url");
        assert_eq!(json["image_url"]["url"], "https://example.com/image.jpg");
        assert_eq!(json["image_url"]["detail"], "high");
    }

    #[test]
    fn test_image_url_deserialization() {
        let json = r#"{
            "type": "image_url",
            "image_url": {
                "url": "https://example.com/test.png",
                "detail": "low"
            }
        }"#;
        let content: Content = serde_json::from_str(json).unwrap();

        match content {
            Content::ImageUrl { image_url } => {
                assert_eq!(image_url.url, "https://example.com/test.png");
                assert_eq!(image_url.detail, Some("low".to_string()));
            },
            _ => panic!("Expected ImageUrl variant"),
        }
    }

    #[test]
    fn test_message_with_text_and_image() {
        let message = Message {
            role: "user".to_string(),
            content: vec![
                Content::text("What's in this image?"),
                Content::image_url_with_detail("https://example.com/photo.jpg", "high"),
            ],
            tool_calls: None,
        };

        let json = serde_json::to_value(&message).unwrap();

        assert_eq!(json["role"], "user");
        assert_eq!(json["content"].as_array().unwrap().len(), 2);
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "What's in this image?");
        assert_eq!(json["content"][1]["type"], "image_url");
        assert_eq!(json["content"][1]["image_url"]["url"], "https://example.com/photo.jpg");
        assert_eq!(json["content"][1]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_completion_request_serialization() {
        let request = CompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                Message::user("Hello!"),
            ],
            temperature: Some(0.7),
            tools: None,
        };

        let json = serde_json::to_value(&request).unwrap();

        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["messages"].as_array().unwrap().len(), 1);
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"][0]["type"], "text");
        assert_eq!(json["messages"][0]["content"][0]["text"], "Hello!");
        // Check temperature exists and is close to 0.7 (floating point precision)
        assert!((json["temperature"].as_f64().unwrap() - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_multimodal_message_serialization() {
        let message = Message {
            role: "user".to_string(),
            content: vec![
                Content::text("Analyze these images:"),
                Content::image_url("https://example.com/img1.jpg"),
                Content::image_url("https://example.com/img2.jpg"),
            ],
            tool_calls: None,
        };

        let json = serde_json::to_value(&message).unwrap();
        let content_array = json["content"].as_array().unwrap();

        assert_eq!(content_array.len(), 3);
        assert_eq!(content_array[0]["type"], "text");
        assert_eq!(content_array[1]["type"], "image_url");
        assert_eq!(content_array[2]["type"], "image_url");
    }
}

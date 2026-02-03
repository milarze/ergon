use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

// Custom deserializer for Message.content field
// Handles OpenAI's flexible content format: string, array, or null
fn deserialize_flexible_content<'de, D>(deserializer: D) -> Result<Vec<Content>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        // String: wrap in Content::Text
        serde_json::Value::String(s) => Ok(vec![Content::Text { text: s }]),
        // Array: deserialize as Vec<Content>
        serde_json::Value::Array(_) => serde_json::from_value(value).map_err(D::Error::custom),
        // Null: return empty vec
        serde_json::Value::Null => Ok(vec![]),
        _ => Err(D::Error::custom("content must be string, array, or null")),
    }
}

// Custom deserializer for Choice.message field
// Handles OpenAI's single message object by wrapping it in a Vec
fn deserialize_message_field<'de, D>(deserializer: D) -> Result<Vec<Message>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        // Single message object: wrap in vec
        serde_json::Value::Object(_) => {
            let msg: Message = serde_json::from_value(value).map_err(D::Error::custom)?;
            Ok(vec![msg])
        }
        // Array: deserialize as Vec<Message>
        serde_json::Value::Array(_) => serde_json::from_value(value).map_err(D::Error::custom),
        _ => Err(D::Error::custom("message must be object or array")),
    }
}

#[derive(Debug, EnumIter, Clone, Default)]
pub enum Clients {
    #[default]
    OpenAI,
    Anthropic,
    Vllm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub id: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub client: Clients,
}

// ImageUrl must be defined before Content since Content references it
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// Content must be defined before Message since the deserializer references it
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    #[serde(rename = "image_url")]
    ImageUrl {
        image_url: ImageUrl,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    #[serde(deserialize_with = "deserialize_flexible_content")]
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl ToString) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl ToString) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl ToString) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(
        tool_use_id: impl ToString,
        content: impl ToString,
        is_error: Option<bool>,
    ) -> Self {
        Self {
            role: "tool".to_string(),
            content: vec![Content::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: content.to_string(),
                is_error,
            }],
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: Some(tool_use_id.to_string()),
        }
    }

    pub fn text_content(&self) -> Vec<&String> {
        self.content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text),
                Content::ToolResult { content, .. } => Some(content),
                _ => None,
            })
            .collect::<Vec<&String>>()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "function", rename_all = "snake_case")]
pub enum Tool {
    Function(Function),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    #[serde(deserialize_with = "deserialize_message_field")]
    pub message: Vec<Message>,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub function: ToolFunction,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallResult {
    pub id: String,
    pub success: bool,
    pub contents: Vec<Content>,
}

impl From<ToolCallResult> for Message {
    fn from(tool_call_result: ToolCallResult) -> Self {
        Self {
            role: "tool".to_string(),
            content: tool_call_result.contents,
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: Some(tool_call_result.id),
        }
    }
}

impl Content {
    pub fn text(content: impl ToString) -> Self {
        Content::Text {
            text: content.to_string(),
        }
    }

    pub fn image_url(url: impl ToString) -> Self {
        Content::ImageUrl {
            image_url: ImageUrl {
                url: url.to_string(),
                detail: None,
            },
        }
    }

    pub fn image_url_with_detail(url: impl ToString, detail: impl ToString) -> Self {
        Content::ImageUrl {
            image_url: ImageUrl {
                url: url.to_string(),
                detail: Some(detail.to_string()),
            },
        }
    }

    pub fn tool_use(id: impl ToString, name: impl ToString, input: serde_json::Value) -> Self {
        Content::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input,
        }
    }

    pub fn tool_result(tool_use_id: impl ToString, content: impl ToString) -> Self {
        Content::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: content.to_string(),
            is_error: None,
        }
    }

    pub fn tool_result_error(tool_use_id: impl ToString, content: impl ToString) -> Self {
        Content::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: content.to_string(),
            is_error: Some(true),
        }
    }

    /// Get the text content if this is a Text variant
    /// This is useful for rendering messages in markdown
    /// It is meant only for rendering purposes
    pub fn as_text(&self) -> Option<String> {
        match self {
            Content::Text { text } => Some(text.clone()),
            Content::ToolUse { id, name, input } => Some(format!(
                "Tool Use - ID: {}, Name: {}, Input: {}",
                id, name, input
            )),
            Content::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                if let Some(true) = is_error {
                    Some(format!(
                        "Tool Result (Error) - Tool Use ID: {}, Content: {}",
                        tool_use_id, content
                    ))
                } else {
                    Some(format!(
                        "Tool Result - Tool Use ID: {}, Content: {}",
                        tool_use_id, content
                    ))
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            }
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
            reasoning_content: None,
            tool_call_id: None,
        };

        let json = serde_json::to_value(&message).unwrap();

        assert_eq!(json["role"], "user");
        assert_eq!(json["content"].as_array().unwrap().len(), 2);
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "What's in this image?");
        assert_eq!(json["content"][1]["type"], "image_url");
        assert_eq!(
            json["content"][1]["image_url"]["url"],
            "https://example.com/photo.jpg"
        );
        assert_eq!(json["content"][1]["image_url"]["detail"], "high");
    }

    #[test]
    fn test_completion_request_serialization() {
        let request = CompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message::user("Hello!")],
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
            reasoning_content: None,
            tool_call_id: None,
        };

        let json = serde_json::to_value(&message).unwrap();
        let content_array = json["content"].as_array().unwrap();

        assert_eq!(content_array.len(), 3);
        assert_eq!(content_array[0]["type"], "text");
        assert_eq!(content_array[1]["type"], "image_url");
        assert_eq!(content_array[2]["type"], "image_url");
    }

    #[test]
    fn test_tool_result_message_serialization() {
        let message = Message::tool_result("tool_use_123", "Tool executed successfully", None);
        let json = serde_json::to_value(&message).unwrap();

        assert_eq!(json["role"], "tool");
        assert_eq!(json["content"][0]["content"], "Tool executed successfully");
        assert_eq!(json["content"][0]["tool_use_id"], "tool_use_123");
        assert_eq!(json["tool_call_id"], "tool_use_123");
    }
}

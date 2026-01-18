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
}

impl Message {
    pub fn system(content: impl ToString) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn user(content: impl ToString) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn assistant(content: impl ToString) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
            reasoning_content: None,
        }
    }

    pub fn tool_result(
        tool_use_id: impl ToString,
        content: impl ToString,
        is_error: Option<bool>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![Content::ToolResult {
                tool_use_id: tool_use_id.to_string(),
                content: content.to_string(),
                is_error,
            }],
            tool_calls: None,
            reasoning_content: None,
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
            role: "assistant".to_string(),
            content: tool_call_result.contents,
            tool_calls: None,
            reasoning_content: None,
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
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        }
    }
}

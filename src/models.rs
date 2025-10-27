use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    pub fn system(content: impl ToString) -> Self {
        Self {
            role: "system".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
        }
    }

    pub fn user(content: impl ToString) -> Self {
        Self {
            role: "user".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl ToString) -> Self {
        Self {
            role: "assistant".to_string(),
            content: vec![Content::text(content)],
            tool_calls: None,
        }
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
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
    pub messages: Vec<Message>,
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

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub contents: Vec<Content>,
}

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
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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

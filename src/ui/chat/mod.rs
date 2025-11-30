use crate::api::clients::{get_model_manager, Clients};
use crate::models::{CompletionRequest, CompletionResponse, Message, ModelInfo};

mod state;
use iced::widget::markdown;
pub use state::State;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender: Sender,
    pub content: String,
    pub markdown_items: Vec<markdown::Item>,
}

impl From<ChatMessage> for Message {
    fn from(chat_message: ChatMessage) -> Self {
        match chat_message.sender {
            Sender::User => Message::user(chat_message.content),
            Sender::Bot => Message::assistant(chat_message.content),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChatAction {
    InputChanged(String),
    SendMessage,
    ResponseReceived(CompletionResponse),
    ModelSelected(String),
    ModelsLoaded(Vec<ModelInfo>),
    UrlClicked(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Sender {
    User,
    Bot,
}

async fn complete_message(
    messages: Vec<ChatMessage>,
    client: Clients,
    model: String,
) -> CompletionResponse {
    let request = CompletionRequest {
        messages: messages.iter().map(|m| m.clone().into()).collect(),
        model,
        temperature: None,
        tools: None,
    };
    let result = client.complete_message(request).await;
    match result {
        Ok(response) => response,
        Err(err) => CompletionResponse {
            id: "error".to_string(),
            object: err.to_string(),
            created: 0,
            model: "".to_string(),
            choices: vec![],
        },
    }
}

async fn load_models() -> Vec<ModelInfo> {
    let manager = get_model_manager();
    match manager.fetch_models().await {
        Ok(_) => {
            match manager.get_models() {
                Ok(models) => models,
                Err(_) => {
                    // Fallback to hardcoded models
                    vec![
                        ModelInfo {
                            name: "gpt-4o-mini".to_string(),
                            id: "gpt-4o-mini".to_string(),
                            client: Clients::OpenAI,
                        },
                        ModelInfo {
                            name: "Claude 3.5 Sonnet".to_string(),
                            id: "claude-3-5-sonnet-20241022".to_string(),
                            client: Clients::Anthropic,
                        },
                    ]
                }
            }
        }
        Err(_) => {
            // Fallback to hardcoded models
            vec![
                ModelInfo {
                    name: "gpt-4o-mini".to_string(),
                    id: "gpt-4o-mini".to_string(),
                    client: Clients::OpenAI,
                },
                ModelInfo {
                    name: "Claude 3.5 Sonnet".to_string(),
                    id: "claude-3-5-sonnet-20241022".to_string(),
                    client: Clients::Anthropic,
                },
            ]
        }
    }
}

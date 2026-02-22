use iced::widget::markdown;

use crate::models::{CompletionResponse, Message, ModelInfo, Tool, ToolCall, ToolCallResult};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message: Message,
    pub markdown_items: Vec<markdown::Item>,
}

impl From<ChatMessage> for Message {
    fn from(chat_message: ChatMessage) -> Self {
        chat_message.message
    }
}

impl From<Message> for ChatMessage {
    fn from(message: Message) -> Self {
        let markdown_items = message
            .content
            .clone()
            .iter()
            .flat_map(|c| {
                match c.as_text() {
                    Some(text) => markdown::parse(&text).collect::<Vec<_>>(),
                    None => markdown::parse("").collect::<Vec<_>>(),
                }
                .into_iter()
            })
            .collect();
        log::info!("Parsed markdown items: {:?}", markdown_items);
        Self {
            markdown_items: markdown_items,
            message,
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
    ToolsLoaaded(Vec<Tool>),
    UrlClicked(String),
    CallTool(ToolCall),
    ToolResponseReceived(Result<ToolCallResult, (String, String)>),
}

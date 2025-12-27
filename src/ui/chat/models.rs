use iced::widget::markdown;

use crate::models::{CompletionResponse, Message, ModelInfo, Tool};

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
    ToolsLoaaded(Vec<Tool>),
    UrlClicked(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Sender {
    User,
    Bot,
}

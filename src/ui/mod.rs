use iced::Element;

mod chat;

#[derive(Debug, Default)]
pub struct Ergon {
    messages: Vec<ChatMessage>,
    input_value: String,
}

#[derive(Debug, Clone)]
struct ChatMessage {
    sender: Sender,
    content: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Sender {
    User,
    Bot,
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    SendMessage,
}

pub fn update(state: &mut Ergon, action: Message) {
    chat::update_chat(state, action);
}

pub fn view(state: &Ergon) -> Element<Message> {
    chat::chat_view(state)
}

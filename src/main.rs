use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

pub fn main() -> iced::Result {
    iced::run("Ergon", update, view)
}

#[derive(Debug, Default)]
struct Ergon {
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
enum Message {
    InputChanged(String),
    SendMessage,
}

fn update(state: &mut Ergon, action: Message) {
    match action {
        Message::InputChanged(value) => {
            state.input_value = value;
        }
        Message::SendMessage => {
            if !state.input_value.is_empty() {
                let user_message = ChatMessage {
                    sender: Sender::User,
                    content: state.input_value.clone(),
                };
                state.messages.push(user_message);

                let bot_response = ChatMessage {
                    sender: Sender::Bot,
                    content: format!("You said: '{}'", state.input_value),
                };
                state.messages.push(bot_response);

                state.input_value.clear();
            }
        }
    }
}

fn view(state: &Ergon) -> Element<Message> {
    let chat_window = column![
        build_message_list(&state.messages),
        build_input_area(&state.input_value),
    ]
    .spacing(10)
    .padding(10);

    container(chat_window)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn build_message_list(messages: &[ChatMessage]) -> Element<Message> {
    let message_column = column(messages.iter().map(|msg| {
        let formatted_message = match msg.sender {
            Sender::User => format!("You: {}", msg.content),
            Sender::Bot => format!("Ergon: {}", msg.content),
        };
        text(formatted_message).into()
    }))
    .spacing(10);

    scrollable(container(message_column).padding(10))
        .height(Length::Fill)
        .into()
}

fn build_input_area(input_value: &str) -> Element<Message> {
    row![
        text_input("Type a message...", input_value)
            .on_input(Message::InputChanged)
            .on_submit(Message::SendMessage),
        button("Send").on_press(Message::SendMessage),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_changed() {
        let mut state = Ergon::default();

        let message = Message::InputChanged("Hello, world!".to_string());
        update(&mut state, message);

        assert_eq!(state.input_value, "Hello, world!");
    }

    #[test]
    fn test_send_message() {
        let mut state = Ergon {
            input_value: "This is a test".to_string(),
            messages: vec![],
        };

        let message = Message::SendMessage;
        update(&mut state, message);

        assert_eq!(state.input_value, "");

        assert_eq!(state.messages.len(), 2);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert_eq!(state.messages[1].sender, Sender::Bot);
        assert_eq!(state.messages[1].content, "You said: 'This is a test'");
    }

    #[test]
    fn test_send_empty_message() {
        let mut state = Ergon::default();

        let message = Message::SendMessage;
        update(&mut state, message);

        assert!(state.messages.is_empty());
    }
}

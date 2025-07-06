use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

#[derive(Debug, Default, Clone)]
pub struct State {
    messages: Vec<ChatMessage>,
    input_value: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    sender: Sender,
    content: String,
}

#[derive(Debug, Clone)]
pub enum Action {
    InputChanged(String),
    SendMessage,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Sender {
    User,
    Bot,
}

impl State {
    pub fn update(&mut self, action: Action) {
        match action {
            Action::InputChanged(value) => {
                self.input_value = value;
            }
            Action::SendMessage => {
                if !self.input_value.is_empty() {
                    let user_message = ChatMessage {
                        sender: Sender::User,
                        content: self.input_value.clone(),
                    };
                    self.messages.push(user_message);

                    let bot_response = ChatMessage {
                        sender: Sender::Bot,
                        content: format!("You said: '{}'", self.input_value),
                    };
                    self.messages.push(bot_response);

                    self.input_value.clear();
                }
            }
        }
    }

    pub fn view(&self) -> Element<Action> {
        let chat_window = column![
            build_message_list(&self.messages),
            build_input_area(&self.input_value),
        ]
        .spacing(10)
        .padding(10);

        container(chat_window)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn build_message_list(messages: &[ChatMessage]) -> Element<Action> {
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

fn build_input_area(input_value: &str) -> Element<Action> {
    row![
        text_input("Type a message...", input_value)
            .on_input(Action::InputChanged)
            .on_submit(Action::SendMessage),
        button("Send").on_press(Action::SendMessage),
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
        let mut state = State::default();

        let message = Action::InputChanged("Hello, world!".to_string());
        state.update(message);

        assert_eq!(state.input_value, "Hello, world!");
    }

    #[test]
    fn test_send_message() {
        let mut state = State {
            input_value: "This is a test".to_string(),
            messages: vec![],
        };

        let message = Action::SendMessage;
        state.update(message);

        assert_eq!(state.input_value, "");

        assert_eq!(state.messages.len(), 2);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert_eq!(state.messages[1].sender, Sender::Bot);
        assert_eq!(state.messages[1].content, "You said: 'This is a test'");
    }

    #[test]
    fn test_send_empty_message() {
        let mut state = State::default();

        let message = Action::SendMessage;
        state.update(message);

        assert!(state.messages.is_empty());
    }
}

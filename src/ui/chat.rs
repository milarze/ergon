use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Task};

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
    ResponseReceived(Result<String, String>),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Sender {
    User,
    Bot,
}

impl State {
    pub fn update(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::InputChanged(value) => {
                self.input_value = value;
                Task::none()
            }
            Action::SendMessage => {
                println!("Sending message: {}", self.input_value);
                if !self.input_value.is_empty() {
                    let user_message = ChatMessage {
                        sender: Sender::User,
                        content: self.input_value.clone(),
                    };
                    self.messages.push(user_message);

                    Task::perform(
                        complete_message(self.messages.clone()),
                        Action::ResponseReceived,
                    )
                } else {
                    Task::none()
                }
            }
            Action::ResponseReceived(response) => {
                println!("Response received: {:?}", response);
                if let Ok(message) = response {
                    let bot_message = ChatMessage {
                        sender: Sender::Bot,
                        content: message,
                    };
                    self.messages.push(bot_message);
                } else {
                    println!("Error receiving response: {:?}", response);
                }
                self.input_value.clear();
                Task::none()
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

async fn complete_message(messages: Vec<ChatMessage>) -> Result<String, String> {
    println!("Completing message with: {:?}", messages);
    Ok(format!(
        "Message sent: {}",
        messages.last().map_or("", |msg| &msg.content)
    ))
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
    use iced::futures::executor::block_on;

    #[test]
    fn test_input_changed() {
        let mut state = State::default();

        let message = Action::InputChanged("Hello, world!".to_string());
        let _ = state.update(message);

        assert_eq!(state.input_value, "Hello, world!");
    }

    async fn mock_complete_message(_messages: Vec<ChatMessage>) -> Result<String, String> {
        Ok("Mocked response".to_string())
    }

    #[test]
    fn test_send_message() {
        let mut state = State {
            input_value: "This is a test".to_string(),
            messages: vec![],
        };

        let message = Action::SendMessage;
        let _ = state.update(message);
        let result_action = block_on(async { mock_complete_message(state.messages.clone()).await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert!(matches!(result_action, Ok(_)));
    }

    async fn mock_failt_complete_message() -> Result<String, String> {
        Err("Mocked bot response".to_string())
    }

    #[test]
    fn test_send_message_error() {
        let mut state = State {
            input_value: "This is a test".to_string(),
            messages: vec![],
        };

        let message = Action::SendMessage;
        let _ = state.update(message);
        let result_action = block_on(async { mock_failt_complete_message().await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert!(matches!(result_action, Err(_)));
    }

    #[test]
    fn test_send_empty_message() {
        let mut state = State::default();

        let message = Action::SendMessage;
        let _ = state.update(message);

        assert!(state.messages.is_empty());
    }

    #[test]
    fn test_response_received() {
        let mut state = State {
            input_value: "Hello".to_string(),
            messages: vec![ChatMessage {
                sender: Sender::User,
                content: "Hello".to_string(),
            }],
        };

        let response = Action::ResponseReceived(Ok("Hi there!".to_string()));
        let _ = state.update(response);

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[1].sender, Sender::Bot);
        assert_eq!(state.messages[1].content, "Hi there!");
        assert!(state.input_value.is_empty());
    }

    #[test]
    fn test_response_received_error() {
        let mut state = State {
            input_value: "Hello".to_string(),
            messages: vec![ChatMessage {
                sender: Sender::User,
                content: "Hello".to_string(),
            }],
        };

        let response = Action::ResponseReceived(Err("Error occurred".to_string()));
        let _ = state.update(response);

        assert_eq!(state.messages.len(), 1);
        assert!(state.input_value.is_empty());
    }
}

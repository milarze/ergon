use crate::api::clients::{get_model_manager, Clients};
use crate::models::{CompletionRequest, CompletionResponse, Message, ModelInfo};
use anyhow::Result;
use iced::widget::{
    button, column, container, markdown, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length, Task, Theme};

#[derive(Debug, Default, Clone)]
pub struct State {
    messages: Vec<ChatMessage>,
    input_value: String,
    selected_model: Option<String>,
    available_models: Vec<ModelInfo>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender: Sender,
    pub content: String,
    pub markdown_items: Vec<markdown::Item>,
}

impl Into<Message> for ChatMessage {
    fn into(self) -> Message {
        match self.sender {
            Sender::User => Message::user(self.content),
            Sender::Bot => Message::assistant(self.content),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
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

impl State {
    pub fn new() -> (Self, Task<Action>) {
        let state = Self::default();
        let task = Task::perform(load_models(), Action::ModelsLoaded);
        (state, task)
    }

    pub fn update(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::InputChanged(value) => {
                self.input_value = value;
                Task::none()
            }
            Action::SendMessage => {
                log::info!("Sending message: {}", self.input_value);
                if !self.input_value.is_empty() {
                    let user_message = ChatMessage {
                        sender: Sender::User,
                        content: self.input_value.clone(),
                        markdown_items: markdown::parse(&self.input_value).collect(),
                    };
                    self.messages.push(user_message);

                    if let Some(model_name) = &self.selected_model {
                        if let Some(model) =
                            get_model_manager().find_model(model_name).unwrap_or(None)
                        {
                            Task::perform(
                                complete_message(
                                    self.messages.clone(),
                                    model.client.clone(),
                                    model.id.clone(),
                                ),
                                Action::ResponseReceived,
                            )
                        } else {
                            // Fallback to default model if selected model not found
                            Task::perform(
                                complete_message(
                                    self.messages.clone(),
                                    Clients::OpenAI,
                                    "gpt-4o-mini".to_string(),
                                ),
                                Action::ResponseReceived,
                            )
                        }
                    } else {
                        // No model selected, use default
                        Task::perform(
                            complete_message(
                                self.messages.clone(),
                                Clients::OpenAI,
                                "gpt-4o-mini".to_string(),
                            ),
                            Action::ResponseReceived,
                        )
                    }
                } else {
                    Task::none()
                }
            }
            Action::ResponseReceived(response) => {
                log::info!("Response received: {:?}", response);
                let messages = if !response.choices.is_empty() {
                    response.choices[0]
                        .messages
                        .iter()
                        .map(|m| m.content.clone())
                        .collect()
                } else {
                    vec!["Error: No response from model.".to_string()]
                };
                let bot_messages = messages.into_iter().map(|content| ChatMessage {
                    sender: Sender::Bot,
                    markdown_items: markdown::parse(&content).collect(),
                    content,
                });
                self.messages.append(&mut bot_messages.collect::<Vec<_>>());
                self.input_value.clear();
                Task::none()
            }
            Action::ModelSelected(model_name) => {
                log::info!("Model selected: {}", model_name);
                self.selected_model = Some(model_name);
                Task::none()
            }
            Action::ModelsLoaded(models) => {
                log::info!("Models loaded: {} models available", models.len());
                self.available_models = models;
                if self.selected_model.is_none() && !self.available_models.is_empty() {
                    self.selected_model = Some(self.available_models[0].name.clone());
                }
                Task::none()
            }
            Action::UrlClicked(url) => {
                log::info!("URL clicked: {}", url);
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Action> {
        let chat_window = column![build_message_list(&self.messages), build_input_area(self),]
            .spacing(10)
            .padding(10);

        container(chat_window)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
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

fn build_message_list(messages: &[ChatMessage]) -> Element<'_, Action> {
    let rows: Vec<Element<Action>> = messages.iter().map(build_message_row).collect();

    scrollable(
        container(column(rows).spacing(10).padding(10))
            .width(Length::Fill)
            .padding(10),
    )
    .height(Length::Fill)
    .into()
}

fn build_message_row(msg: &ChatMessage) -> Element<'_, Action> {
    let formatted_message = match msg.sender {
        Sender::User => "You: ".to_string(),
        Sender::Bot => "Bot: ".to_string(),
    };

    row![
        text(formatted_message),
        markdown(
            &msg.markdown_items,
            markdown::Settings::default(),
            markdown::Style::from_palette(Theme::default().palette())
        )
        .map(|url| Action::UrlClicked(url.to_string())),
    ]
    .into()
}

fn build_input_area(state: &State) -> Element<'_, Action> {
    row![
        text_input("Type a message...", &state.input_value)
            .on_input(Action::InputChanged)
            .on_submit(Action::SendMessage)
            .width(Length::FillPortion(8)),
        button("Send")
            .on_press(Action::SendMessage)
            .width(Length::FillPortion(1)),
        pick_list(
            state
                .available_models
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>(),
            state.selected_model.as_ref(),
            Action::ModelSelected
        )
        .width(Length::FillPortion(3)),
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
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
        };

        let message = Action::SendMessage;
        let _ = state.update(message);
        let result_action = block_on(async { mock_complete_message(state.messages.clone()).await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert!(result_action.is_ok());
    }

    async fn mock_failt_complete_message() -> Result<String, String> {
        Err("Mocked bot response".to_string())
    }

    #[test]
    fn test_send_message_error() {
        let mut state = State {
            input_value: "This is a test".to_string(),
            messages: vec![],
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
        };

        let message = Action::SendMessage;
        let _ = state.update(message);
        let result_action = block_on(async { mock_failt_complete_message().await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert!(result_action.is_err());
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
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
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
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
        };

        let response = Action::ResponseReceived(Err("Error occurred".to_string()));
        let _ = state.update(response);

        assert_eq!(state.messages.len(), 1);
        assert!(state.input_value.is_empty());
    }

    #[test]
    fn test_model_selection() {
        let mut state = State::default();
        let model_name = "gpt-4o-mini".to_string();

        let action = Action::ModelSelected(model_name.clone());
        let _ = state.update(action);

        assert_eq!(state.selected_model, Some(model_name));
    }
}

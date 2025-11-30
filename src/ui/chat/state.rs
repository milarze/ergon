use iced::{
    widget::{button, column, container, markdown, pick_list, row, scrollable, text, text_input},
    Alignment, Element, Length, Task, Theme,
};

use crate::{
    api::clients::get_model_manager,
    models::{Clients, CompletionResponse, ModelInfo},
    ui::chat::{complete_message, load_models, ChatAction, ChatMessage, Sender},
};

#[derive(Debug, Default, Clone)]
pub struct State {
    messages: Vec<ChatMessage>,
    input_value: String,
    awaiting_response: bool,
    selected_model: Option<String>,
    available_models: Vec<ModelInfo>,
}

impl State {
    pub fn new() -> (Self, Task<ChatAction>) {
        let state = State {
            awaiting_response: true,
            ..Default::default()
        };
        let task = Task::perform(load_models(), ChatAction::ModelsLoaded);
        (state, task)
    }

    pub fn update(&mut self, action: ChatAction) -> Task<ChatAction> {
        match action {
            ChatAction::InputChanged(value) => self.on_input_changed(value),
            ChatAction::SendMessage => self.on_send_message(),
            ChatAction::ResponseReceived(response) => self.on_response_received(response),
            ChatAction::ModelSelected(model_name) => self.on_model_selected(model_name),
            ChatAction::ModelsLoaded(models) => self.on_models_loaded(models),
            ChatAction::UrlClicked(url) => self.on_url_clicked(url),
        }
    }

    fn on_input_changed(&mut self, value: String) -> Task<ChatAction> {
        self.input_value = value;
        Task::none()
    }

    fn on_send_message(&mut self) -> Task<ChatAction> {
        self.awaiting_response = true;
        if !self.input_value.is_empty() {
            let user_message = self.build_pending_message();
            self.messages.push(user_message);

            let default_model = "gpt-4o-mini".to_string();
            let model_name = self.selected_model.as_ref().unwrap_or(&default_model);
            let model = get_model_manager()
                .find_model(model_name)
                .unwrap_or(None)
                .unwrap_or(ModelInfo {
                    name: "gpt-4o-mini".to_string(),
                    id: "gpt-4o-mini".to_string(),
                    client: Clients::OpenAI,
                });
            Task::perform(
                complete_message(
                    self.messages.clone(),
                    model.client.clone(),
                    model.id.clone(),
                ),
                ChatAction::ResponseReceived,
            )
        } else {
            Task::none()
        }
    }

    fn build_pending_message(&self) -> ChatMessage {
        ChatMessage {
            sender: Sender::User,
            content: self.input_value.clone(),
            markdown_items: markdown::parse(&self.input_value).collect(),
        }
    }

    fn on_response_received(&mut self, response: CompletionResponse) -> Task<ChatAction> {
        log::info!("Response received: {:?}", response);
        let response_messages = if !response.choices.is_empty() {
            response.choices[0]
                .message
                .iter()
                .flat_map(|m| {
                    m.content
                        .iter()
                        .filter_map(|c| c.as_text().map(String::from))
                })
                .collect()
        } else {
            vec!["Error: No response from model.".to_string()]
        };
        let bot_messages = response_messages.into_iter().map(|content| ChatMessage {
            sender: Sender::Bot,
            markdown_items: markdown::parse(&content).collect(),
            content,
        });
        self.messages.append(&mut bot_messages.collect::<Vec<_>>());
        self.input_value.clear();
        self.awaiting_response = false;
        Task::none()
    }

    fn on_model_selected(&mut self, model_name: String) -> Task<ChatAction> {
        self.selected_model = Some(model_name);
        Task::none()
    }

    fn on_models_loaded(&mut self, models: Vec<ModelInfo>) -> Task<ChatAction> {
        self.available_models = models;
        if self.selected_model.is_none() && !self.available_models.is_empty() {
            self.selected_model = Some(self.available_models[0].name.clone());
        }
        self.awaiting_response = false;
        Task::none()
    }

    fn on_url_clicked(&mut self, url: String) -> Task<ChatAction> {
        log::info!("URL clicked: {}", url);
        Task::none()
    }

    pub fn view(&self) -> Element<'_, ChatAction> {
        let chat_window = column![self.build_message_list(), self.build_input_area(),]
            .spacing(10)
            .padding(10);

        container(chat_window)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_message_list(&self) -> Element<'_, ChatAction> {
        let rows: Vec<Element<ChatAction>> =
            self.messages.iter().map(Self::build_message_row).collect();

        scrollable(
            container(column(rows).spacing(10).padding(10))
                .width(Length::Fill)
                .padding(10),
        )
        .height(Length::Fill)
        .into()
    }

    fn build_message_row(msg: &ChatMessage) -> Element<'_, ChatAction> {
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
            .map(|url| ChatAction::UrlClicked(url.to_string())),
        ]
        .into()
    }

    fn build_input_area(&self) -> Element<'_, ChatAction> {
        row![
            text_input("Type a message...", &self.input_value)
                .on_input_maybe(if self.awaiting_response {
                    None
                } else {
                    Some(ChatAction::InputChanged)
                })
                .on_submit(ChatAction::SendMessage)
                .width(Length::FillPortion(8)),
            button("Send")
                .on_press_maybe(if self.awaiting_response {
                    None
                } else {
                    Some(ChatAction::SendMessage)
                })
                .width(Length::FillPortion(1)),
            pick_list(
                self.available_models
                    .iter()
                    .map(|m| m.name.clone())
                    .collect::<Vec<_>>(),
                self.selected_model.as_ref(),
                ChatAction::ModelSelected
            )
            .width(Length::FillPortion(3)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }
}

#[cfg(test)]
mod tests {

    use crate::models::CompletionResponse;

    use super::*;
    use anyhow::Result;
    use iced::futures::executor::block_on;

    #[test]
    fn test_input_changed() {
        let mut state = State::default();

        let message = ChatAction::InputChanged("Hello, world!".to_string());
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
            awaiting_response: false,
        };

        let message = ChatAction::SendMessage;
        let _ = state.update(message);
        assert!(state.awaiting_response);
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
            awaiting_response: false,
        };

        let message = ChatAction::SendMessage;
        let _ = state.update(message);
        assert!(state.awaiting_response);
        let result_action = block_on(async { mock_failt_complete_message().await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].sender, Sender::User);
        assert_eq!(state.messages[0].content, "This is a test");

        assert!(result_action.is_err());
    }

    #[test]
    fn test_send_empty_message() {
        let mut state = State::default();

        let message = ChatAction::SendMessage;
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
            awaiting_response: true,
        };

        let response = ChatAction::ResponseReceived(CompletionResponse {
            id: "resp1".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4o-mini".to_string(),
            choices: vec![crate::models::Choice {
                index: 0,
                message: vec![crate::models::Message::assistant("Hi there!".to_string())],
                finish_reason: "stop".to_string(),
            }],
        });
        let _ = state.update(response);

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[1].sender, Sender::Bot);
        assert_eq!(state.messages[1].content, "Hi there!");
        assert!(state.input_value.is_empty());
        assert!(!state.awaiting_response);
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
            awaiting_response: true,
        };

        let response = ChatAction::ResponseReceived(CompletionResponse {
            id: "error".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4o-mini".to_string(),
            choices: vec![],
        });
        let _ = state.update(response);

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[1].sender, Sender::Bot);
        assert_eq!(state.messages[1].content, "Error: No response from model.");
        assert!(state.input_value.is_empty());
        assert!(!state.awaiting_response);
    }

    #[test]
    fn test_model_selection() {
        let mut state = State::default();
        let model_name = "gpt-4o-mini".to_string();

        let action = ChatAction::ModelSelected(model_name.clone());
        let _ = state.update(action);

        assert_eq!(state.selected_model, Some(model_name));
    }
}

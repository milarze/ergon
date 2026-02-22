use std::collections::HashSet;

use iced::{
    widget::{
        button, column, container, markdown, pick_list, row, scrollable, text, text_input, Row,
    },
    Alignment, Element,
    Length::{self, Fill, Shrink},
    Task, Theme,
};

use crate::{
    api::clients::get_model_manager,
    models::{Clients, CompletionResponse, Message, ModelInfo, Tool, ToolCall, ToolCallResult},
    ui::chat::{
        call_tool, complete_message, load_models, load_tools, models::ChatMessage, ChatAction,
    },
};

#[derive(Debug, Default, Clone)]
pub struct State {
    messages: Vec<ChatMessage>,
    input_value: String,
    awaiting_response: bool,
    selected_model: Option<String>,
    available_models: Vec<ModelInfo>,
    available_tools: Vec<Tool>,
    pending_tool_calls: HashSet<String>,
}

impl State {
    pub fn new() -> (Self, Task<ChatAction>) {
        let state = State {
            awaiting_response: true,
            ..Default::default()
        };
        let task = Task::batch([
            Task::perform(load_models(), ChatAction::ModelsLoaded),
            Task::perform(load_tools(), ChatAction::ToolsLoaaded),
        ]);
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
            ChatAction::ToolsLoaaded(tools) => self.on_tools_loaded(tools),
            ChatAction::CallTool(tool_call) => self.on_tool_called(tool_call),
            ChatAction::ToolResponseReceived(response) => self.on_tool_response_received(response),
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
        }

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
                self.available_tools.clone(),
            ),
            ChatAction::ResponseReceived,
        )
    }

    fn build_pending_message(&self) -> ChatMessage {
        ChatMessage {
            message: Message::user(self.input_value.clone()),
            markdown_items: markdown::parse(&self.input_value).collect(),
        }
    }

    fn on_response_received(&mut self, response: CompletionResponse) -> Task<ChatAction> {
        let choices = &response.choices;
        self.input_value.clear();
        if choices.is_empty() {
            self.messages
                .push(Message::assistant("Error: No response from model.".to_string()).into());
            self.input_value.clear();
            self.awaiting_response = false;
            return Task::none();
        }
        self.messages.append(
            choices[0]
                .message
                .iter()
                .map(|m| m.clone().into())
                .collect::<Vec<_>>()
                .as_mut(),
        );
        let tool_calls = self.get_response_tool_calls(choices);
        if !tool_calls.is_empty() {
            tool_calls.iter().for_each(|tool_call| {
                self.pending_tool_calls.insert(tool_call.id.clone());
            });
            Task::batch(
                tool_calls
                    .into_iter()
                    .map(|tool_call| Task::perform(async move { tool_call }, ChatAction::CallTool)),
            )
        } else {
            self.awaiting_response = false;
            Task::none()
        }
    }

    fn get_response_tool_calls(&self, choices: &[crate::models::Choice]) -> Vec<ToolCall> {
        if !choices.is_empty() {
            choices[0]
                .message
                .iter()
                .flat_map(|m| m.tool_calls.clone().unwrap_or(vec![]))
                .collect::<Vec<_>>()
        } else {
            vec![]
        }
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

    fn on_tools_loaded(&mut self, tools: Vec<crate::models::Tool>) -> Task<ChatAction> {
        self.available_tools = tools;
        Task::none()
    }

    fn on_tool_called(&mut self, tool_call: ToolCall) -> Task<ChatAction> {
        Task::perform(call_tool(tool_call), ChatAction::ToolResponseReceived)
    }

    fn on_tool_response_received(
        &mut self,
        response: Result<ToolCallResult, (String, String)>,
    ) -> Task<ChatAction> {
        match response {
            Ok(result) => {
                self.pending_tool_calls.remove(&result.id);
                let message: Message = result.into();
                self.messages.push(message.into())
            }
            Err((call_id, error_message)) => {
                log::error!("Tool call failed: {}", error_message);
                self.pending_tool_calls.remove(&call_id);
                self.messages
                    .push(Message::tool_result(call_id, error_message, Some(true)).into())
            }
        }
        if self.pending_tool_calls.is_empty() {
            self.on_send_message()
        } else {
            Task::none()
        }
    }

    fn on_url_clicked(&mut self, url: String) -> Task<ChatAction> {
        log::info!("URL clicked: {}", url);
        Task::none()
    }

    pub fn view<'a>(&'a self, theme: &'a Theme) -> Element<'a, ChatAction> {
        let chat_window = column![self.build_message_list(theme), self.build_input_area(),]
            .spacing(10)
            .padding(10);

        container(chat_window)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn build_message_list<'a>(&'a self, theme: &'a Theme) -> Element<'a, ChatAction> {
        let rows: Vec<Element<ChatAction>> = self
            .messages
            .iter()
            .map(|msg| Self::build_message_row(&msg.message.role, msg, theme))
            .collect();

        scrollable(
            container(column(rows).spacing(10).padding(10))
                .width(Length::Fill)
                .padding(10),
        )
        .height(Length::Fill)
        .into()
    }

    fn build_message_row<'a>(
        role: &'a str,
        message: &'a ChatMessage,
        theme: &'a Theme,
    ) -> Element<'a, ChatAction> {
        let align = match role {
            "user" => Alignment::End,
            _ => Alignment::Start,
        };
        let color = match role {
            "user" => theme.palette().primary,
            "assistant" => theme.palette().text,
            _ => theme.palette().background,
        };
        let role_widget: container::Container<'_, ChatAction, _, _> =
            container(text(role).color(color))
                .width(Shrink)
                .align_x(align);
        let content_widget: container::Container<'_, ChatAction, _, _> = container(
            markdown(
                &message.markdown_items,
                markdown::Settings::with_style(markdown::Style::from_palette(theme.palette())),
            )
            .map(|url| ChatAction::UrlClicked(url.to_string())),
        )
        .width(Fill)
        .align_x(align);
        let mut elements = vec![];
        match role {
            "user" => {
                elements.push(content_widget.into());
                elements.push(role_widget.into());
            }
            "assistant" | "tool" => {
                elements.push(role_widget.into());
                elements.push(content_widget.into());
            }
            _ => {}
        }

        Row::from_vec(elements).spacing(20).width(Fill).into()
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
            available_tools: vec![],
            awaiting_response: false,
            pending_tool_calls: HashSet::new(),
        };

        let message = ChatAction::SendMessage;
        let _ = state.update(message);
        assert!(state.awaiting_response);
        let result_action = block_on(async { mock_complete_message(state.messages.clone()).await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].message.role, "user");
        assert_eq!(
            state.messages[0].message.text_content().first(),
            Some(&&"This is a test".to_string())
        );

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
            available_tools: vec![],
            awaiting_response: false,
            pending_tool_calls: HashSet::new(),
        };

        let message = ChatAction::SendMessage;
        let _ = state.update(message);
        assert!(state.awaiting_response);
        let result_action = block_on(async { mock_failt_complete_message().await });

        assert_eq!(state.messages.len(), 1);

        assert_eq!(state.messages[0].message.role, "user");
        assert_eq!(
            state.messages[0].message.text_content().first(),
            Some(&&"This is a test".to_string())
        );

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
                message: Message::user("Hello".to_string()),
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: true,
            pending_tool_calls: HashSet::new(),
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
        assert_eq!(state.messages[1].message.role, "assistant");
        assert_eq!(
            state.messages[1].message.text_content().first(),
            Some(&&"Hi there!".to_string())
        );
        assert!(state.input_value.is_empty());
        assert!(!state.awaiting_response);
    }

    #[test]
    fn test_response_received_error() {
        let mut state = State {
            input_value: "Hello".to_string(),
            messages: vec![ChatMessage {
                message: Message::user("Hello".to_string()),
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some("gpt-4o-mini".to_string()),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: true,
            pending_tool_calls: HashSet::new(),
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
        assert_eq!(state.messages[1].message.role, "assistant");
        assert_eq!(
            state.messages[1].message.text_content().first(),
            Some(&&"Error: No response from model.".to_string())
        );
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

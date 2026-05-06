use std::collections::HashSet;

use base64::Engine as _;

use iced::{
    futures::{stream, StreamExt},
    widget::{
        button, column, container, markdown, pick_list, row, scrollable, text, text_input, Row,
    },
    Alignment, Element,
    Length::{self, Fill, Shrink},
    Subscription, Task, Theme,
};
use iced_aw::Spinner;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    acp::{get_agent_manager, AgentEvent, AgentUpdate, AuthMethodInfo, AvailableCommand, StopReason},
    api::clients::get_model_manager,
    config::Config,
    models::{
        Clients, CompletionResponse, FileData, Message, ModelInfo, Tool, ToolCall, ToolCallResult,
    },
    ui::chat::{
        call_tool, complete_message, load_models, load_tools, models::ChatMessage, prompt_agent,
        start_agent,
        tasks::{
            authenticate_agent, current_session_info, persist_agent_session, resume_agent,
            AgentPromptOutcome, AgentResumeOutcome, AgentStartOutcome,
        },
        ChatAction, ChatTarget,
    },
};

#[derive(Debug, Default, Clone)]
pub struct State {
    messages: Vec<ChatMessage>,
    input_value: String,
    awaiting_response: bool,
    selected_model: Option<ModelInfo>,
    available_models: Vec<ModelInfo>,
    available_tools: Vec<Tool>,
    pending_tool_calls: HashSet<String>,
    files: Option<Vec<FileData>>,

    // ── ACP agent path ────────────────────────────────────────────────
    /// Where the next prompt is routed. Defaults to LLM.
    pub chat_target: ChatTarget,
    /// Names of agents currently configured (mirrored from `Config::acp_agents`).
    available_agents: Vec<String>,
    /// The assistant message currently being streamed by the active agent
    /// turn, if any. We keep its index into `messages` so successive
    /// `AgentMessageChunk`s append to the same bubble.
    streaming_agent_message: Option<usize>,
    /// Auth methods advertised by the active agent. Non-empty means we are
    /// waiting for the user to pick a sign-in method; the input area renders
    /// per-method buttons in this state.
    pending_auth_methods: Vec<AuthMethodInfo>,
    /// Slash commands most recently advertised by the active agent. Rendered
    /// as a chip row above the input. Cleared when switching targets.
    available_commands: Vec<AvailableCommand>,
    /// Index of the chat bubble currently rendering the agent's plan, if any.
    /// Each `Plan` update from the agent is the *complete* current plan, so
    /// we replace this bubble's contents in place rather than appending.
    plan_message_index: Option<usize>,
}

impl State {
    pub fn new() -> (Self, Task<ChatAction>) {
        let available_agents: Vec<String> = Config::default()
            .acp_agents
            .iter()
            .map(|a| a.name().to_string())
            .collect();
        let state = State {
            awaiting_response: true,
            available_agents,
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
            ChatAction::OpenFileDialog => self.on_open_file_dialog(),
            ChatAction::FileSelected(path_buffer) => self.on_file_selected(path_buffer),
            ChatAction::TargetSelected(target) => self.on_target_selected(target),
            ChatAction::AgentStarted(result) => self.on_agent_started(result),
            ChatAction::AgentEvent(event) => self.on_agent_event(event),
            ChatAction::AgentPromptComplete(result) => self.on_agent_prompt_complete(result),
            ChatAction::AuthenticateAgent { agent, method_id } => {
                self.on_authenticate_agent(agent, method_id)
            }
            ChatAction::AgentAuthenticated {
                agent,
                method_id,
                result,
            } => self.on_agent_authenticated(agent, method_id, result),
            ChatAction::SlashCommandSelected(name) => self.on_slash_command_selected(name),
            ChatAction::ResumeAgent { agent } => self.on_resume_agent(agent),
            ChatAction::AgentResumed { agent, result } => self.on_agent_resumed(agent, result),
            ChatAction::PersistAgentSession(info) => self.on_persist_agent_session(info),
        }
    }

    fn on_input_changed(&mut self, value: String) -> Task<ChatAction> {
        self.input_value = value;
        Task::none()
    }

    fn on_send_message(&mut self) -> Task<ChatAction> {
        // Route based on chat target.
        match self.chat_target.clone() {
            ChatTarget::Llm => self.on_send_message_llm(),
            ChatTarget::Agent(name) => self.on_send_message_agent(name),
        }
    }

    fn on_send_message_llm(&mut self) -> Task<ChatAction> {
        self.awaiting_response = true;
        if !self.input_value.is_empty() {
            let user_message = self.build_pending_message();
            self.messages.push(user_message);
        }

        if self.selected_model.is_none() {
            log::error!("No model selected, cannot send message");
            self.awaiting_response = false;
            return Task::none();
        }

        let model = get_model_manager()
            .find_model(&self.selected_model.as_ref().unwrap().name)
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

    fn on_send_message_agent(&mut self, agent_name: String) -> Task<ChatAction> {
        self.awaiting_response = true;
        let prompt_text = std::mem::take(&mut self.input_value);
        if prompt_text.is_empty() {
            self.awaiting_response = false;
            return Task::none();
        }
        // Render the user message immediately.
        self.messages
            .push(ChatMessage::from_role_and_text("user", prompt_text.clone()));
        // Reset streaming pointer; the next AgentMessageChunk will create a
        // fresh assistant bubble.
        self.streaming_agent_message = None;

        // Ensure the process is running, then send the prompt. `prompt_agent`
        // lazily creates the session, so an `auth_required` will surface as
        // `AgentPromptOutcome::AuthRequired`.
        let agent = agent_name.clone();
        Task::perform(
            async move {
                start_agent(agent.clone()).await?;
                prompt_agent(agent, prompt_text).await
            },
            ChatAction::AgentPromptComplete,
        )
    }

    fn on_target_selected(&mut self, target: ChatTarget) -> Task<ChatAction> {
        self.chat_target = target.clone();
        // Drop any per-target accumulated state when switching.
        self.available_commands.clear();
        self.pending_auth_methods.clear();
        self.plan_message_index = None;
        // If switching to an agent, kick off `start_agent` so the
        // subscription gets a live broadcast receiver and any auth-required
        // banner gets rendered up front.
        match target {
            ChatTarget::Agent(name) => Task::perform(start_agent(name), ChatAction::AgentStarted),
            ChatTarget::Llm => Task::none(),
        }
    }

    fn on_slash_command_selected(&mut self, name: String) -> Task<ChatAction> {
        // Replace the input contents with `/<name> `; if the user had typed
        // something else, it gets dropped (the chip click is an explicit
        // re-selection of the next prompt).
        self.input_value = format!("/{name} ");
        Task::none()
    }

    fn on_agent_started(
        &mut self,
        result: Result<AgentStartOutcome, String>,
    ) -> Task<ChatAction> {
        match result {
            Ok(AgentStartOutcome::Ready) => {
                log::info!("ACP agent ready");
                // Capture and persist the freshly-allocated session id so a
                // future "Resume last session" works across restarts.
                if let ChatTarget::Agent(name) = &self.chat_target {
                    let agent_name = name.clone();
                    return Task::perform(
                        current_session_info(agent_name),
                        ChatAction::PersistAgentSession,
                    );
                }
            }
            Ok(AgentStartOutcome::AuthRequired(methods)) => {
                self.push_auth_required_bubble(methods);
            }
            Err(err) => {
                log::error!("Failed to start ACP agent: {}", err);
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Agent failed to start:** {err}"),
                ));
                self.awaiting_response = false;
                // Fall back to LLM mode so the input doesn't lock up.
                self.chat_target = ChatTarget::Llm;
            }
        }
        Task::none()
    }

    fn on_persist_agent_session(
        &mut self,
        info: Option<crate::ui::chat::tasks::AgentSessionInfo>,
    ) -> Task<ChatAction> {
        match info {
            Some(info) => Task::perform(persist_agent_session(info), |()| {
                ChatAction::PersistAgentSession(None)
            }),
            None => Task::none(),
        }
    }

    fn on_resume_agent(&mut self, agent: String) -> Task<ChatAction> {
        // Look up the stored session for this agent; if none, no-op.
        let cfg = Config::default();
        let stored = match cfg.acp_session_state.get(&agent) {
            Some(s) => s.clone(),
            None => {
                log::warn!("ResumeAgent: no stored session for '{}'", agent);
                return Task::none();
            }
        };
        self.awaiting_response = true;
        let agent_for_msg = agent.clone();
        Task::perform(
            resume_agent(agent, stored.session_id, stored.workspace_root),
            move |result| ChatAction::AgentResumed {
                agent: agent_for_msg.clone(),
                result,
            },
        )
    }

    fn on_agent_resumed(
        &mut self,
        agent: String,
        result: Result<AgentResumeOutcome, String>,
    ) -> Task<ChatAction> {
        self.awaiting_response = false;
        match result {
            Ok(AgentResumeOutcome::Resumed) => {
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Resumed previous session** for `{agent}`."),
                ));
                Task::none()
            }
            Ok(AgentResumeOutcome::Unsupported) => {
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    "**Resume unsupported:** this agent does not advertise `load_session`."
                        .to_string(),
                ));
                Task::none()
            }
            Ok(AgentResumeOutcome::WorkspaceMismatch) => {
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    "**Resume skipped:** stored session was created in a different workspace."
                        .to_string(),
                ));
                Task::none()
            }
            Ok(AgentResumeOutcome::AuthRequired(methods)) => {
                self.push_auth_required_bubble(methods);
                Task::none()
            }
            Err(err) => {
                log::error!("resume_agent({agent}) failed: {err}");
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Failed to resume session:** {err}"),
                ));
                Task::none()
            }
        }
    }

    /// Render an auth-required notice as a chat bubble. The actual
    /// "Sign in with X" buttons are rendered as part of the message in
    /// `messages_view` (a special role discriminator is used).
    ///
    /// Implementation note: iced's markdown widget can't render interactive
    /// buttons inline. For v1 we render the method list as text and surface
    /// real buttons in the input area while in this state. To avoid a
    /// second piece of UI state, we encode the methods in a dedicated field.
    fn push_auth_required_bubble(&mut self, methods: Vec<AuthMethodInfo>) {
        let body = if methods.is_empty() {
            "**Authentication required**, but the agent did not advertise any methods.".to_string()
        } else {
            let lines: Vec<String> = methods
                .iter()
                .map(|m| match &m.description {
                    Some(d) => format!("- **{}** — {}", m.name, d),
                    None => format!("- **{}**", m.name),
                })
                .collect();
            format!(
                "**Authentication required.** Sign-in options:\n{}",
                lines.join("\n")
            )
        };
        self.messages
            .push(ChatMessage::from_role_and_text("assistant", body));
        self.pending_auth_methods = methods;
        self.awaiting_response = false;
    }

    fn on_authenticate_agent(
        &mut self,
        agent: String,
        method_id: String,
    ) -> Task<ChatAction> {
        self.awaiting_response = true;
        let agent_for_msg = agent.clone();
        let method_for_msg = method_id.clone();
        Task::perform(
            authenticate_agent(agent, method_id),
            move |result| ChatAction::AgentAuthenticated {
                agent: agent_for_msg.clone(),
                method_id: method_for_msg.clone(),
                result,
            },
        )
    }

    fn on_agent_authenticated(
        &mut self,
        agent: String,
        method_id: String,
        result: Result<(), String>,
    ) -> Task<ChatAction> {
        self.awaiting_response = false;
        match result {
            Ok(()) => {
                log::info!("Authenticated agent '{}' with method '{}'", agent, method_id);
                self.pending_auth_methods.clear();
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Authenticated** with `{method_id}`."),
                ));
                // Retry session creation now that auth succeeded.
                Task::perform(start_agent(agent), ChatAction::AgentStarted)
            }
            Err(err) => {
                log::error!("authenticate({method_id}) failed: {err}");
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Authentication failed (`{method_id}`):** {err}"),
                ));
                Task::none()
            }
        }
    }

    fn on_agent_event(&mut self, event: AgentEvent) -> Task<ChatAction> {
        match event {
            AgentEvent::Update(update) => self.apply_agent_update(update),
            AgentEvent::Fatal(msg) => {
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Agent error:** {msg}"),
                ));
                self.awaiting_response = false;
                self.streaming_agent_message = None;
            }
        }
        Task::none()
    }

    fn apply_agent_update(&mut self, update: AgentUpdate) {
        match update {
            AgentUpdate::AgentMessage(chunk) => {
                self.append_streaming_assistant("assistant", &chunk);
            }
            AgentUpdate::AgentThought(chunk) => {
                // Render thoughts as a separate role so they're visually distinct.
                self.append_streaming_assistant("thought", &chunk);
            }
            AgentUpdate::ToolCall { id, title, kind } => {
                self.streaming_agent_message = None;
                self.messages.push(ChatMessage::from_role_and_text(
                    "tool",
                    format!("**[{kind}]** {title}  \n_(id: `{id}`)_"),
                ));
            }
            AgentUpdate::ToolCallUpdate {
                id,
                status,
                content_summary,
            } => {
                let status = status.unwrap_or_else(|| "update".to_string());
                let body = match content_summary {
                    Some(c) => format!("`{id}` → {status}: {c}"),
                    None => format!("`{id}` → {status}"),
                };
                self.messages
                    .push(ChatMessage::from_role_and_text("tool", body));
            }
            AgentUpdate::Plan { entries } => {
                let body = if entries.is_empty() {
                    "**Plan** _(empty)_".to_string()
                } else {
                    let lines: Vec<String> = entries
                        .iter()
                        .map(|e| {
                            format!(
                                "{} `[{}]` {}",
                                e.status.glyph(),
                                e.priority.label(),
                                e.content
                            )
                        })
                        .collect();
                    format!("**Plan**\n{}", lines.join("\n"))
                };
                // Replace existing plan bubble in place if we have one;
                // otherwise push a fresh one and remember its index. Each
                // Plan update is the full current plan, not a delta.
                match self.plan_message_index {
                    Some(idx) if idx < self.messages.len() => {
                        if let Some(msg) = self.messages.get_mut(idx) {
                            *msg = ChatMessage::from_role_and_text("plan", body);
                        }
                    }
                    _ => {
                        self.messages
                            .push(ChatMessage::from_role_and_text("plan", body));
                        self.plan_message_index = Some(self.messages.len() - 1);
                    }
                }
                // A plan bubble is its own thing — break the streaming
                // assistant chain so the next text chunk starts a new bubble.
                self.streaming_agent_message = None;
            }
            AgentUpdate::AvailableCommands(cmds) => {
                log::info!(
                    "Agent advertised {} command(s): {:?}",
                    cmds.len(),
                    cmds.iter().map(|c| &c.name).collect::<Vec<_>>()
                );
                self.available_commands = cmds;
            }
            AgentUpdate::ModeChanged(m) => {
                log::info!("Agent mode changed: {}", m);
            }
            AgentUpdate::Other(text) => {
                log::debug!("Agent other update: {}", text);
            }
        }
    }

    fn append_streaming_assistant(&mut self, role: &str, chunk: &str) {
        // If there's an in-flight streaming bubble of this role, append to it.
        if let Some(idx) = self.streaming_agent_message {
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.message.role == role {
                    msg.append_text(chunk);
                    return;
                }
            }
        }
        // Otherwise start a new bubble.
        self.messages
            .push(ChatMessage::from_role_and_text(role, chunk));
        self.streaming_agent_message = Some(self.messages.len() - 1);
    }

    fn on_agent_prompt_complete(
        &mut self,
        result: Result<AgentPromptOutcome, String>,
    ) -> Task<ChatAction> {
        self.awaiting_response = false;
        self.streaming_agent_message = None;
        // The plan is per-turn; new turns begin with a fresh bubble.
        self.plan_message_index = None;
        match result {
            Ok(AgentPromptOutcome::Completed(outcome)) => {
                if !matches!(outcome.stop_reason, StopReason::EndTurn) {
                    log::info!("Agent stopped: {:?}", outcome.stop_reason);
                }
            }
            Ok(AgentPromptOutcome::AuthRequired(methods)) => {
                self.push_auth_required_bubble(methods);
            }
            Err(err) => {
                log::error!("Agent prompt failed: {}", err);
                self.messages.push(ChatMessage::from_role_and_text(
                    "assistant",
                    format!("**Agent prompt failed:** {err}"),
                ));
            }
        }
        Task::none()
    }

    fn build_pending_message(&self) -> ChatMessage {
        ChatMessage {
            message: Message::user(self.input_value.clone(), self.files.clone()),
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
        self.selected_model = self
            .available_models
            .iter()
            .find(|m| m.name == model_name)
            .cloned();
        Task::none()
    }

    fn on_models_loaded(&mut self, models: Vec<ModelInfo>) -> Task<ChatAction> {
        self.available_models = models;
        if (self.selected_model.is_none() && !self.available_models.is_empty()) ||
        !self.available_models.contains(self.selected_model.as_ref().unwrap()) {
            self.selected_model = Some(self.available_models[0].clone());
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

    /// Name of the agent currently selected as the chat target, if any.
    pub fn active_agent_name(&self) -> Option<&str> {
        match &self.chat_target {
            ChatTarget::Agent(name) => Some(name),
            ChatTarget::Llm => None,
        }
    }

    /// Refresh the list of agents from `Config`. Called when settings save.
    pub fn refresh_available_agents(&mut self) {
        self.available_agents = Config::default()
            .acp_agents
            .iter()
            .map(|a| a.name().to_string())
            .collect();
        // If the selected agent disappeared, drop it.
        if let ChatTarget::Agent(name) = &self.chat_target {
            if !self.available_agents.contains(name) {
                self.chat_target = ChatTarget::Llm;
            }
        }
    }

    fn on_open_file_dialog(&mut self) -> Task<ChatAction> {
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .add_filter("All files", &["*"])
                    .pick_files()
                    .await
                    .map(|files| {
                        files
                            .into_iter()
                            .map(|file| file.path().to_path_buf())
                            .collect::<Vec<_>>()
                    })
            },
            ChatAction::FileSelected,
        )
    }

    fn on_file_selected(
        &mut self,
        path_buffer: Option<Vec<std::path::PathBuf>>,
    ) -> Task<ChatAction> {
        if let Some(paths) = path_buffer {
            const BASE64_ENGINE: base64::engine::general_purpose::GeneralPurpose =
                base64::engine::GeneralPurpose::new(
                    &base64::alphabet::STANDARD,
                    base64::engine::general_purpose::PAD,
                );
            if self.files.is_none() {
                self.files = Some(vec![]);
            }
            let file_infos: Vec<FileData> = paths
                .iter()
                .filter_map(|path| {
                    log::info!("Selected file: {}", path.display());
                    let mime_type = mime_guess::from_path(path)
                        .first_or_octet_stream()
                        .essence_str()
                        .to_string();
                    let file_data = match std::fs::read(path) {
                        Ok(data) => {
                            let base64_content = BASE64_ENGINE.encode(&data);
                            Some(format!("data:{};base64,{}", mime_type, base64_content))
                        }
                        Err(err) => {
                            log::error!("Failed to read file {}: {}", path.display(), err);
                            None
                        }
                    };
                    file_data.map(|data| FileData {
                        filename: Some(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        ),
                        file_data: Some(data),
                        file_id: None,
                    })
                })
                .collect();
            if let Some(files) = &mut self.files {
                files.extend(file_infos);
            }
        } else {
            log::info!("File selection cancelled");
        }
        Task::none()
    }

    /// Subscription that streams [`AgentEvent`]s from the active ACP session,
    /// if any. Each event is mapped to [`ChatAction::AgentEvent`].
    pub fn subscription(&self) -> Subscription<ChatAction> {
        match &self.chat_target {
            ChatTarget::Agent(name) => {
                Subscription::run_with(name.clone(), agent_event_subscription)
            }
            ChatTarget::Llm => Subscription::none(),
        }
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
        // Build the list of available chat targets.
        let mut targets: Vec<ChatTarget> = vec![ChatTarget::Llm];
        targets.extend(
            self.available_agents
                .iter()
                .cloned()
                .map(ChatTarget::Agent),
        );

        let target_picker = pick_list(
            targets,
            Some(self.chat_target.clone()),
            ChatAction::TargetSelected,
        )
        .width(Length::FillPortion(4));

        // Show the model picker only in LLM mode; in Agent mode the agent owns
        // its model.
        let model_picker: Element<'_, ChatAction> = if matches!(self.chat_target, ChatTarget::Llm) {
            pick_list(
                self.available_models
                    .iter()
                    .map(|m| m.name.clone())
                    .collect::<Vec<_>>(),
                self.selected_model.as_ref().map(|m| m.name.clone()),
                ChatAction::ModelSelected,
            )
            .width(Length::FillPortion(4))
            .into()
        } else {
            container(text("(agent-managed)"))
                .width(Length::FillPortion(4))
                .into()
        };

        let main_row = row![
            text_input("Type a message...", &self.input_value)
                .on_input_maybe(if self.awaiting_response {
                    None
                } else {
                    Some(ChatAction::InputChanged)
                })
                .on_submit(ChatAction::SendMessage)
                .width(Length::FillPortion(10)),
            button("📁")
                .on_press(ChatAction::OpenFileDialog)
                .width(Length::FillPortion(1)),
            self.build_send_button(),
            target_picker,
            model_picker,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Auth row: only present when there are advertised auth methods for
        // the active agent and no auth attempt is currently in flight.
        let auth_row = self.build_auth_row();
        let cmd_row = self.build_slash_command_row();
        let resume_row = self.build_resume_row();

        let mut col = column![].spacing(8);
        if let Some(rr) = resume_row {
            col = col.push(rr);
        }
        if let Some(ar) = auth_row {
            col = col.push(ar);
        }
        if let Some(cr) = cmd_row {
            col = col.push(cr);
        }
        col.push(main_row).into()
    }

    /// Build a "Resume last session" row when the active agent has a stored
    /// session id and is not currently in an auth-required state. Returns
    /// `None` otherwise.
    fn build_resume_row(&self) -> Option<Element<'_, ChatAction>> {
        let agent = match &self.chat_target {
            ChatTarget::Agent(name) => name.clone(),
            ChatTarget::Llm => return None,
        };
        if !self.pending_auth_methods.is_empty() {
            return None;
        }
        // Check stored session presence (cheap: Config::default reads the
        // settings file but this view is only re-rendered on state changes).
        let cfg = Config::default();
        let stored = cfg.acp_session_state.get(&agent)?;
        let label = format!(
            "Resume last session ({}…)",
            stored.session_id.chars().take(8).collect::<String>()
        );
        let mut btn = button(text(label));
        if !self.awaiting_response {
            btn = btn.on_press(ChatAction::ResumeAgent { agent });
        }
        let row_widgets: Row<'_, ChatAction> = Row::new()
            .spacing(10)
            .align_y(Alignment::Center)
            .push(btn);
        Some(row_widgets.into())
    }

    /// Build a horizontal chip row with one button per advertised slash
    /// command. Returns `None` outside of agent mode or when no commands are
    /// advertised.
    fn build_slash_command_row(&self) -> Option<Element<'_, ChatAction>> {
        if self.available_commands.is_empty()
            || matches!(self.chat_target, ChatTarget::Llm)
        {
            return None;
        }
        let mut row_widgets: Row<'_, ChatAction> = Row::new().spacing(6).align_y(Alignment::Center);
        row_widgets = row_widgets.push(text("Commands:"));
        for cmd in &self.available_commands {
            let name = cmd.name.clone();
            let label = match &cmd.input_hint {
                Some(h) if !h.is_empty() => format!("/{} ⟨{}⟩", cmd.name, h),
                _ => format!("/{}", cmd.name),
            };
            let mut btn = button(text(label));
            if !self.awaiting_response {
                btn = btn.on_press(ChatAction::SlashCommandSelected(name));
            }
            row_widgets = row_widgets.push(btn);
        }
        Some(scrollable(row_widgets).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::default(),
        )).into())
    }

    /// Build the "Sign in with X" button row when the active agent has
    /// reported an auth-required state. Returns `None` outside of agent mode
    /// or when there are no pending auth methods.
    fn build_auth_row(&self) -> Option<Element<'_, ChatAction>> {
        if self.pending_auth_methods.is_empty() {
            return None;
        }
        let agent = match &self.chat_target {
            ChatTarget::Agent(name) => name.clone(),
            ChatTarget::Llm => return None,
        };

        let mut row_widgets: Row<'_, ChatAction> = Row::new().spacing(10).align_y(Alignment::Center);
        row_widgets = row_widgets.push(text("Sign in:"));
        for method in &self.pending_auth_methods {
            let label = format!("{} ({})", method.name, method.id);
            let agent_for = agent.clone();
            let method_id = method.id.clone();
            let mut btn = button(text(label));
            if !self.awaiting_response {
                btn = btn.on_press(ChatAction::AuthenticateAgent {
                    agent: agent_for,
                    method_id,
                });
            }
            row_widgets = row_widgets.push(btn);
        }
        Some(row_widgets.into())
    }

    fn build_send_button(&self) -> Element<'_, ChatAction> {
        let button_content = if self.awaiting_response {
            container(Spinner::new())
        } else {
            container(text("Send"))
        };

        button(button_content.width(Length::Fill).center_x(Length::Fill))
            .on_press_maybe(if self.awaiting_response {
                None
            } else {
                Some(ChatAction::SendMessage)
            })
            .width(Length::FillPortion(2))
            .into()
    }
}

/// Build a stream of [`ChatAction::AgentEvent`]s for the named agent.
///
/// Used as the `builder` argument to [`Subscription::run_with`]. We poll the
/// agent manager every 100ms until the session exists, then subscribe to its
/// broadcast and forward events. If the session disappears (e.g. user
/// shutdown), the stream ends.
fn agent_event_subscription(agent_name: &String) -> impl iced::futures::Stream<Item = ChatAction> {
    let name = agent_name.clone();
    stream::unfold(
        AgentSubState::WaitingForSession { name, attempts: 0 },
        |st| async move {
            match st {
                AgentSubState::WaitingForSession { name, attempts } => {
                    let manager = get_agent_manager();
                    match manager.get(&name) {
                        Ok(Some(handle)) => {
                            let receiver = handle.subscribe();
                            let mut bs = BroadcastStream::new(receiver);
                            // Wait for the first event to dodge a one-cycle gap.
                            let first = bs.next().await;
                            match first {
                                Some(Ok(ev)) => Some((
                                    ChatAction::AgentEvent(ev),
                                    AgentSubState::Streaming { stream: bs },
                                )),
                                Some(Err(_)) | None => None,
                            }
                        }
                        _ => {
                            // Backoff before retrying. After ~30 s give up.
                            if attempts > 300 {
                                return None;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            Some((
                                ChatAction::AgentEvent(AgentEvent::Update(AgentUpdate::Other(
                                    String::new(),
                                ))),
                                AgentSubState::WaitingForSession {
                                    name,
                                    attempts: attempts + 1,
                                },
                            ))
                        }
                    }
                }
                AgentSubState::Streaming { mut stream } => match stream.next().await {
                    Some(Ok(ev)) => Some((
                        ChatAction::AgentEvent(ev),
                        AgentSubState::Streaming { stream },
                    )),
                    Some(Err(_)) | None => None,
                },
            }
        },
    )
    // Filter out the synthetic "still waiting" empty events.
    .filter(|action| {
        let keep = !matches!(
            action,
            ChatAction::AgentEvent(AgentEvent::Update(AgentUpdate::Other(s))) if s.is_empty()
        );
        async move { keep }
    })
}

enum AgentSubState {
    WaitingForSession {
        name: String,
        attempts: u32,
    },
    Streaming {
        stream: BroadcastStream<AgentEvent>,
    },
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
            selected_model: Some(ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: false,
            pending_tool_calls: HashSet::new(),
            files: None,
            chat_target: ChatTarget::Llm,
            available_agents: vec![],
            streaming_agent_message: None,
            pending_auth_methods: Vec::new(),
            available_commands: Vec::new(),
            plan_message_index: None,
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
            selected_model: Some(ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: false,
            pending_tool_calls: HashSet::new(),
            files: None,
            chat_target: ChatTarget::Llm,
            available_agents: vec![],
            streaming_agent_message: None,
            pending_auth_methods: Vec::new(),
            available_commands: Vec::new(),
            plan_message_index: None,
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
                message: Message::user("Hello".to_string(), None),
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some(ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: true,
            pending_tool_calls: HashSet::new(),
            files: None,
            chat_target: ChatTarget::Llm,
            available_agents: vec![],
            streaming_agent_message: None,
            pending_auth_methods: Vec::new(),
            available_commands: Vec::new(),
            plan_message_index: None,
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
                message: Message::user("Hello".to_string(), None),
                markdown_items: markdown::parse("Hello").collect(),
            }],
            selected_model: Some(ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }),
            available_models: vec![ModelInfo {
                name: "gpt-4o-mini".to_string(),
                id: "gpt-4o-mini".to_string(),
                client: Clients::OpenAI,
            }],
            available_tools: vec![],
            awaiting_response: true,
            pending_tool_calls: HashSet::new(),
            files: None,
            chat_target: ChatTarget::Llm,
            available_agents: vec![],
            streaming_agent_message: None,
            pending_auth_methods: Vec::new(),
            available_commands: Vec::new(),
            plan_message_index: None,
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
        let mut state = State {
            available_models: vec![
                ModelInfo {
                    name: "gpt-4o-mini".to_string(),
                    id: "gpt-4o-mini".to_string(),
                    client: Clients::OpenAI,
                },
                ModelInfo {
                    name: "gpt-3.5-turbo".to_string(),
                    id: "gpt-3.5-turbo".to_string(),
                    client: Clients::OpenAI,
                },
            ],
            ..State::default()
        };
        let model_name = "gpt-4o-mini".to_string();

        let action = ChatAction::ModelSelected(model_name.clone());
        let _ = state.update(action);

        assert_eq!(state.selected_model, Some(ModelInfo {
            name: model_name.clone(),
            id: model_name,
            client: Clients::OpenAI,
        }));
    }

    #[test]
    fn test_file_selection() {
        let mut state = State::default();
        let file_path = std::path::PathBuf::from("/path/to/file.txt");

        let action = ChatAction::FileSelected(Some(vec![file_path.clone()]));
        let _ = state.update(action);

        // Not reading actual files. The file reader defaults to None if it can't read the file.
        assert_eq!(state.files, Some(vec![]));
    }
}

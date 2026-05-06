use std::path::PathBuf;

use iced::widget::markdown;

use crate::acp::AgentEvent;
use crate::models::{CompletionResponse, Message, ModelInfo, Tool, ToolCall, ToolCallResult};
use crate::ui::chat::tasks::{AgentPromptOutcome, AgentStartOutcome};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message: Message,
    pub markdown_items: Vec<markdown::Item>,
}

impl ChatMessage {
    /// Build a ChatMessage with the given role and raw text. Used by the
    /// agent path where we don't go through the `Message` constructors.
    pub fn from_role_and_text(role: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let message = Message {
            role: role.into(),
            content: vec![crate::models::Content::text(text.clone())],
            tool_calls: None,
            reasoning_content: None,
            tool_call_id: None,
        };
        Self {
            markdown_items: markdown::parse(&text).collect(),
            message,
        }
    }

    /// Append more text to the underlying message and re-parse markdown.
    /// Used for streaming agent message chunks.
    pub fn append_text(&mut self, more: &str) {
        // Find the first text content; append to it. Otherwise push new text.
        let mut appended = false;
        for c in self.message.content.iter_mut() {
            if let crate::models::Content::Text { text } = c {
                text.push_str(more);
                appended = true;
                break;
            }
        }
        if !appended {
            self.message
                .content
                .push(crate::models::Content::text(more.to_string()));
        }
        // Re-parse all text content combined for markdown rendering.
        let all_text: String = self
            .message
            .content
            .iter()
            .filter_map(|c| c.as_text())
            .collect::<Vec<_>>()
            .join("\n");
        self.markdown_items = markdown::parse(&all_text).collect();
    }
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
            markdown_items,
            message,
        }
    }
}

/// Where prompts from the chat input are routed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum ChatTarget {
    /// Standard LLM provider via the existing `Clients` enum.
    #[default]
    Llm,
    /// External ACP agent identified by its configured name.
    Agent(String),
}

impl std::fmt::Display for ChatTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatTarget::Llm => write!(f, "LLM"),
            ChatTarget::Agent(name) => write!(f, "Agent: {name}"),
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
    OpenFileDialog,
    FileSelected(Option<Vec<PathBuf>>),

    // ── ACP agent path ────────────────────────────────────────────────
    /// User picked a chat target (LLM or Agent(name)).
    TargetSelected(ChatTarget),
    /// Agent session start finished. Carries `AuthRequired` if the agent
    /// needs sign-in before a session can be created.
    AgentStarted(Result<AgentStartOutcome, String>),
    /// A streamed event from the running agent session.
    AgentEvent(AgentEvent),
    /// The current agent prompt turn finished. May be `AuthRequired` if the
    /// agent rejected session creation just before the prompt would have run.
    AgentPromptComplete(Result<AgentPromptOutcome, String>),
    /// User clicked a "Sign in with X" button.
    AuthenticateAgent {
        agent: String,
        method_id: String,
    },
    /// `authenticate` request finished.
    AgentAuthenticated {
        agent: String,
        method_id: String,
        result: Result<(), String>,
    },
    /// User clicked a slash-command chip; insert "/<name> " into the input.
    SlashCommandSelected(String),
    /// User clicked the "Resume last session" button. Triggers `resume_agent`
    /// for the named agent using the stored session id from `Config`.
    ResumeAgent { agent: String },
    /// `resume_agent` finished.
    AgentResumed {
        agent: String,
        result: Result<crate::ui::chat::tasks::AgentResumeOutcome, String>,
    },
    /// Result of fetching session info for persistence after a session was
    /// (re)created. `None` means no live session, in which case the stored
    /// entry (if any) is left untouched.
    PersistAgentSession(Option<crate::ui::chat::tasks::AgentSessionInfo>),
}

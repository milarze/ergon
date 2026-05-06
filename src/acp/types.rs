//! Ergon-facing wrapper types for ACP.
//!
//! These types insulate the rest of the codebase from the upstream
//! `agent_client_protocol` crate so changes there only ripple through `crate::acp`.

use agent_client_protocol::schema as acp_schema;
use serde::{Deserialize, Serialize};

/// Information about an authentication method advertised by an agent.
///
/// Re-shape of `acp_schema::AuthMethod` for the stable `Agent` variant only
/// (which is the only stable variant in the spec at v0.11). Unstable variants
/// (`EnvVar`, `Terminal`) are folded into this same shape on a best-effort
/// basis using their advertised id/name/description.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthMethodInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<&acp_schema::AuthMethod> for AuthMethodInfo {
    fn from(m: &acp_schema::AuthMethod) -> Self {
        AuthMethodInfo {
            id: m.id().0.to_string(),
            name: m.name().to_string(),
            description: m.description().map(str::to_string),
        }
    }
}

/// A slash command advertised by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AvailableCommand {
    pub name: String,
    pub description: String,
    /// Hint for free-form input the user is expected to type after the
    /// command name (`None` if the command takes no arguments).
    pub input_hint: Option<String>,
}

/// A streamed update from an ACP agent during a prompt turn.
///
/// This is a thin re-shape of `acp_schema::SessionUpdate` exposing only the
/// fields the rest of Ergon needs to render. Variants we do not care about
/// today are folded into `Other` so the UI never has to know about them.
#[derive(Debug, Clone)]
pub enum AgentUpdate {
    /// Streaming text from the agent's reply.
    AgentMessage(String),
    /// Streaming "thought" / reasoning text.
    AgentThought(String),
    /// A new tool call has begun.
    ToolCall {
        id: String,
        title: String,
        kind: String,
    },
    /// A tool call's status or output changed.
    ToolCallUpdate {
        id: String,
        status: Option<String>,
        content_summary: Option<String>,
    },
    /// The agent published a plan / checklist.
    Plan {
        entries: Vec<PlanEntry>,
    },
    /// Available slash commands changed.
    AvailableCommands(Vec<AvailableCommand>),
    /// The session's mode changed.
    ModeChanged(String),
    /// Anything else the UI doesn't render directly.
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    pub status: PlanEntryStatus,
    pub priority: PlanEntryPriority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
    Other,
}

impl PlanEntryStatus {
    /// Unicode glyph used to render the entry as a checkbox-like marker.
    pub fn glyph(self) -> &'static str {
        match self {
            PlanEntryStatus::Pending => "☐",
            PlanEntryStatus::InProgress => "▶",
            PlanEntryStatus::Completed => "✅",
            PlanEntryStatus::Other => "•",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
    Other,
}

impl PlanEntryPriority {
    pub fn label(self) -> &'static str {
        match self {
            PlanEntryPriority::High => "high",
            PlanEntryPriority::Medium => "med",
            PlanEntryPriority::Low => "low",
            PlanEntryPriority::Other => "?",
        }
    }
}

/// Outcome of a prompt turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
    Other(String),
}

impl From<acp_schema::StopReason> for StopReason {
    fn from(reason: acp_schema::StopReason) -> Self {
        match reason {
            acp_schema::StopReason::EndTurn => Self::EndTurn,
            acp_schema::StopReason::MaxTokens => Self::MaxTokens,
            acp_schema::StopReason::MaxTurnRequests => Self::MaxTurnRequests,
            acp_schema::StopReason::Refusal => Self::Refusal,
            acp_schema::StopReason::Cancelled => Self::Cancelled,
            other => Self::Other(format!("{other:?}")),
        }
    }
}

/// Convert an upstream `SessionUpdate` into the Ergon-facing form.
pub fn map_session_update(update: acp_schema::SessionUpdate) -> AgentUpdate {
    use acp_schema::SessionUpdate as SU;
    match update {
        SU::AgentMessageChunk(chunk) => AgentUpdate::AgentMessage(content_chunk_text(&chunk)),
        SU::AgentThoughtChunk(chunk) => AgentUpdate::AgentThought(content_chunk_text(&chunk)),
        SU::UserMessageChunk(chunk) => {
            AgentUpdate::Other(format!("user: {}", content_chunk_text(&chunk)))
        }
        SU::ToolCall(tc) => AgentUpdate::ToolCall {
            id: tc.tool_call_id.0.to_string(),
            title: tc.title.clone(),
            kind: format!("{:?}", tc.kind),
        },
        SU::ToolCallUpdate(update) => AgentUpdate::ToolCallUpdate {
            id: update.tool_call_id.0.to_string(),
            status: update.fields.status.map(|s| format!("{s:?}")),
            content_summary: update
                .fields
                .content
                .as_ref()
                .map(|c| format!("{} item(s)", c.len())),
        },
        SU::Plan(plan) => AgentUpdate::Plan {
            entries: plan
                .entries
                .into_iter()
                .map(|e| PlanEntry {
                    content: e.content,
                    status: match e.status {
                        acp_schema::PlanEntryStatus::Pending => PlanEntryStatus::Pending,
                        acp_schema::PlanEntryStatus::InProgress => PlanEntryStatus::InProgress,
                        acp_schema::PlanEntryStatus::Completed => PlanEntryStatus::Completed,
                        _ => PlanEntryStatus::Other,
                    },
                    priority: match e.priority {
                        acp_schema::PlanEntryPriority::High => PlanEntryPriority::High,
                        acp_schema::PlanEntryPriority::Medium => PlanEntryPriority::Medium,
                        acp_schema::PlanEntryPriority::Low => PlanEntryPriority::Low,
                        _ => PlanEntryPriority::Other,
                    },
                })
                .collect(),
        },
        SU::AvailableCommandsUpdate(av) => AgentUpdate::AvailableCommands(
            av.available_commands
                .into_iter()
                .map(|c| {
                    let input_hint = c.input.as_ref().map(|i| match i {
                        acp_schema::AvailableCommandInput::Unstructured(u) => u.hint.clone(),
                        _ => String::new(),
                    });
                    AvailableCommand {
                        name: c.name,
                        description: c.description,
                        input_hint,
                    }
                })
                .collect(),
        ),
        SU::CurrentModeUpdate(m) => AgentUpdate::ModeChanged(m.current_mode_id.0.to_string()),
        other => AgentUpdate::Other(format!("{other:?}")),
    }
}

fn content_chunk_text(chunk: &acp_schema::ContentChunk) -> String {
    use acp_schema::ContentBlock;
    match &chunk.content {
        ContentBlock::Text(t) => t.text.clone(),
        ContentBlock::Image(_) => "[image]".to_string(),
        ContentBlock::Audio(_) => "[audio]".to_string(),
        ContentBlock::Resource(_) => "[resource]".to_string(),
        ContentBlock::ResourceLink(link) => format!("[link: {}]", link.uri),
        other => format!("{other:?}"),
    }
}

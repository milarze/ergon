//! ACP (Agent Client Protocol) integration.
//!
//! Ergon acts as the *client* (editor) side of ACP. External agent processes
//! drive conversations and call back into Ergon for filesystem reads, writes,
//! terminal execution, and permission requests.
//!
//! Layout:
//! - [`types`] – Ergon-facing shapes, isolating the rest of the codebase
//!   from the upstream protocol crate.
//! - [`fs`] – sandboxed filesystem callbacks.
//! - [`terminal`] – `terminal/*` callbacks.
//! - [`permissions`] – permission-request bridging to the UI.
//! - [`session`] – session lifecycle, command channel, update fan-out.
//! - [`transport`] – spawning external agents over stdio.
//! - [`manager`] – global registry of running agents (mirrors `mcp::ToolManager`).
//!
//! NOTE: Phase 1 lays down the plumbing. The UI integration in Phase 3 will
//! exercise these symbols; until then `#[allow(dead_code)]` keeps the
//! warnings out of the way.
#![allow(dead_code)]

pub mod fs;
pub mod manager;
pub mod mcp_passthrough;
pub mod permissions;
pub mod session;
pub mod terminal;
pub mod transport;
pub mod types;

pub use manager::{get_agent_manager, AgentManager};
pub use session::{AgentEvent, AgentSessionHandle, PromptOutcome};
pub use types::{
    AgentUpdate, AuthMethodInfo, AvailableCommand, PlanEntry, PlanEntryPriority, PlanEntryStatus,
    StopReason,
};

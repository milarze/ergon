//! Permission-request bridging.
//!
//! When an ACP agent calls `session/request_permission`, Ergon must surface the
//! choice to the user and round-trip the answer. This module owns the
//! request → UI → response plumbing.
//!
//! The current implementation stores a *policy* per agent. Phase 3 will replace
//! the auto-approval default with an actual UI modal.

use agent_client_protocol::schema::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome,
};
use serde::{Deserialize, Serialize};

/// How permission requests for an agent should be resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PermissionPolicy {
    /// Always pick the first non-denying option (YOLO).
    #[default]
    AutoApprove,
    /// Always cancel.
    AlwaysDeny,
    /// Surface a UI prompt (requires the UI subscription wiring; until then
    /// behaves the same as `AutoApprove`).
    Prompt,
}

/// Resolve a permission request without UI interaction. Used as the default
/// during early phases; Phase 3 will replace this with channel-based prompts.
pub fn resolve_request(
    request: &RequestPermissionRequest,
    policy: &PermissionPolicy,
) -> RequestPermissionResponse {
    match policy {
        PermissionPolicy::AlwaysDeny => {
            RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
        }
        PermissionPolicy::AutoApprove | PermissionPolicy::Prompt => {
            if let Some(opt) = pick_default_option(&request.options) {
                RequestPermissionResponse::new(RequestPermissionOutcome::Selected(
                    SelectedPermissionOutcome::new(opt.option_id.clone()),
                ))
            } else {
                RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
            }
        }
    }
}

fn pick_default_option(options: &[PermissionOption]) -> Option<&PermissionOption> {
    options
        .iter()
        .find(|o| matches!(o.kind, PermissionOptionKind::AllowOnce))
        .or_else(|| {
            options
                .iter()
                .find(|o| matches!(o.kind, PermissionOptionKind::AllowAlways))
        })
        .or_else(|| options.first())
}

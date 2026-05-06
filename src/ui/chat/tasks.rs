use rmcp::model::JsonObject;
use serde_json::Value;

use crate::{
    acp::{get_agent_manager, AuthMethodInfo, PromptOutcome},
    api::clients::get_model_manager,
    models::{
        Clients, CompletionRequest, CompletionResponse, Content, ModelInfo, Tool, ToolCall,
        ToolCallResult,
    },
    ui::chat::models::ChatMessage,
};

pub async fn complete_message(
    messages: Vec<ChatMessage>,
    client: Clients,
    model: String,
    tools: Vec<Tool>,
) -> CompletionResponse {
    log::info!(
        "message roles: {:?}",
        messages
            .iter()
            .map(|m| m.message.role.clone())
            .collect::<Vec<String>>()
    );
    log::info!(
        "message contents: {:?}",
        messages
            .iter()
            .map(|m| m.message.content.clone())
            .collect::<Vec<Vec<Content>>>()
    );
    let request = CompletionRequest {
        messages: messages.iter().map(|cm| cm.clone().into()).collect(),
        model,
        temperature: None,
        tools: Some(tools),
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

pub async fn load_models() -> Vec<ModelInfo> {
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

pub async fn load_tools() -> Vec<crate::models::Tool> {
    let manager = crate::mcp::get_tool_manager();
    match manager.load_tools().await {
        Ok(_) => manager.get_tools().unwrap_or_default(),
        Err(_) => vec![],
    }
}

pub async fn call_tool(tool_call: ToolCall) -> Result<ToolCallResult, (String, String)> {
    log::info!("Received tool call: {:?}", tool_call);
    let manager = crate::mcp::get_tool_manager();
    let call_id = tool_call.id.clone();
    let client = manager
        .get_client_by_tool_call(&tool_call.function.name)
        .map_err(|e| (call_id.clone(), e.to_string()))?
        .ok_or_else(|| {
            (
                call_id.clone(),
                "Client not found for tool call".to_string(),
            )
        })?;
    let args_json: JsonObject<Value> = serde_json::from_str(&tool_call.function.arguments)
        .map_err(|e| (call_id.clone(), format!("Failed to parse arguments: {}", e)))?;
    log::info!("Tool call arguments as JSON: {:?}", args_json);
    let function_name = tool_call.function.name.clone();
    let (_, client_function_name) = manager
        .tool_client_and_name_by_tool_call(function_name)
        .map_err(|e| {
            (
                call_id.clone(),
                format!("Failed to extract client function name: {}", e),
            )
        })?
        .ok_or_else(|| {
            (
                call_id.clone(),
                "Function name mapping not found for tool call".to_string(),
            )
        })?;
    let request_params = rmcp::model::CallToolRequestParams::new(client_function_name.clone())
        .with_arguments(args_json.clone());
    log::info!(
        "Calling tool: {} with args: {:?}",
        client_function_name,
        request_params.arguments
    );
    let tool_result = client
        .call_tool(request_params)
        .await
        .map_err(|e| (call_id.clone(), e.to_string()))?;
    let json_string = serde_json::to_string(&tool_result).map_err(|e| {
        (
            call_id.clone(),
            format!("Failed to serialize tool result: {}", e),
        )
    })?;
    Ok(ToolCallResult {
        success: true,
        id: call_id.clone(),
        contents: vec![Content::tool_result(call_id, json_string)],
    })
}

// ── ACP agent helpers ─────────────────────────────────────────────────────

/// Result of attempting to start an agent and create a session.
#[derive(Debug, Clone)]
pub enum AgentStartOutcome {
    Ready,
    AuthRequired(Vec<AuthMethodInfo>),
}

/// Ensure an ACP agent process is running and a session is created. If the
/// agent reports `auth_required`, returns the advertised auth methods so the
/// UI can surface a sign-in picker.
pub async fn start_agent(agent_name: String) -> Result<AgentStartOutcome, String> {
    use crate::acp::manager::EnsureSessionError;
    match get_agent_manager().ensure_session(&agent_name).await {
        Ok(_) => Ok(AgentStartOutcome::Ready),
        Err(EnsureSessionError::AuthRequired { methods, .. }) => {
            Ok(AgentStartOutcome::AuthRequired(methods))
        }
        Err(EnsureSessionError::Other(e)) => Err(e.to_string()),
    }
}

/// Result of a prompt call. Either the agent ran the turn and gave us a
/// stop reason, or it told us we need to authenticate first.
#[derive(Debug, Clone)]
pub enum AgentPromptOutcome {
    Completed(PromptOutcome),
    AuthRequired(Vec<AuthMethodInfo>),
}

/// Send a single-turn prompt to a running ACP agent. The agent's streamed
/// updates surface separately via the subscription.
pub async fn prompt_agent(
    agent_name: String,
    text: String,
) -> Result<AgentPromptOutcome, String> {
    use crate::acp::session::SessionError;
    let manager = get_agent_manager();
    let handle = manager
        .get(&agent_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("agent '{agent_name}' is not running"))?;
    match handle.prompt_text(text).await {
        Ok(o) => Ok(AgentPromptOutcome::Completed(o)),
        Err(SessionError::AuthRequired { methods }) => {
            Ok(AgentPromptOutcome::AuthRequired(methods))
        }
        Err(SessionError::Other(e)) => Err(e.to_string()),
    }
}

/// Run an `authenticate` request against the named agent.
pub async fn authenticate_agent(agent_name: String, method_id: String) -> Result<(), String> {
    let manager = get_agent_manager();
    let handle = manager
        .get(&agent_name)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("agent '{agent_name}' is not running"))?;
    handle
        .authenticate(method_id)
        .await
        .map_err(|e| e.to_string())
}

/// Persist the given session info to `~/.ergon/settings.json` under
/// `acp_session_state`. Idempotent. Best-effort: errors are logged.
pub async fn persist_agent_session(info: AgentSessionInfo) {
    use crate::config::{Config, StoredAcpSession};
    // Reload from disk so we don't clobber other concurrent edits.
    let mut cfg = Config::default();
    cfg.acp_session_state.insert(
        info.agent_name.clone(),
        StoredAcpSession {
            session_id: info.session_id.clone(),
            workspace_root: info.workspace_root.clone(),
        },
    );
    cfg.update_settings();
}
#[derive(Debug, Clone)]
pub struct AgentSessionInfo {
    pub agent_name: String,
    pub session_id: String,
    pub workspace_root: String,
}

/// Fetch the current session id + workspace root for a running agent, if any.
/// Returns `None` if the agent is not running or has no live session yet.
pub async fn current_session_info(agent_name: String) -> Option<AgentSessionInfo> {
    let manager = get_agent_manager();
    let handle = manager.get(&agent_name).ok().flatten()?;
    let id = handle.current_session_id().await?;
    Some(AgentSessionInfo {
        agent_name,
        session_id: id.0.to_string(),
        workspace_root: handle.workspace_root.to_string_lossy().into_owned(),
    })
}

/// Outcome of attempting to resume a previously-stored session.
#[derive(Debug, Clone)]
pub enum AgentResumeOutcome {
    /// Session resumed successfully.
    Resumed,
    /// Agent does not advertise `load_session` capability.
    Unsupported,
    /// Stored workspace root no longer matches the agent's current workspace
    /// root, so we declined to resume.
    WorkspaceMismatch,
    /// Agent demanded authentication before we could load the session.
    AuthRequired(Vec<AuthMethodInfo>),
}

/// Spawn the agent (if needed) and attempt to load a previously-stored
/// session. Does NOT call `session/new`.
pub async fn resume_agent(
    agent_name: String,
    stored_session_id: String,
    stored_workspace_root: String,
) -> Result<AgentResumeOutcome, String> {
    use crate::acp::session::SessionError;
    let manager = get_agent_manager();
    let handle = manager
        .ensure_started(&agent_name)
        .await
        .map_err(|e| e.to_string())?;
    if !handle.supports_load_session {
        return Ok(AgentResumeOutcome::Unsupported);
    }
    let current_root = handle.workspace_root.to_string_lossy();
    if current_root != stored_workspace_root {
        return Ok(AgentResumeOutcome::WorkspaceMismatch);
    }
    let session_id = agent_client_protocol::schema::SessionId::new(stored_session_id);
    match handle.load_session(session_id).await {
        Ok(_) => Ok(AgentResumeOutcome::Resumed),
        Err(SessionError::AuthRequired { methods }) => Ok(AgentResumeOutcome::AuthRequired(methods)),
        Err(SessionError::Other(e)) => Err(e.to_string()),
    }
}

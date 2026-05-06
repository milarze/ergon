//! ACP session lifecycle.
//!
//! A [`spawn_session`] call spawns the agent process, runs the ACP `Client`
//! event loop on a tokio task, performs the `initialize` handshake, and
//! returns an [`AgentSessionHandle`]. Session creation (`session/new`) is a
//! separate step ([`AgentSessionHandle::ensure_session`]) so that the auth
//! flow can intercept `auth_required` errors and surface available auth
//! methods to the UI.

use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::schema::{
    AuthenticateRequest, CancelNotification, ClientCapabilities, ContentBlock,
    CreateTerminalRequest, ErrorCode, FileSystemCapabilities, InitializeRequest,
    InitializeResponse, KillTerminalRequest, LoadSessionRequest, McpServer, NewSessionRequest,
    PromptRequest, ProtocolVersion, ReadTextFileRequest, ReleaseTerminalRequest,
    RequestPermissionRequest, SessionId, SessionNotification, TerminalOutputRequest, TextContent,
    WaitForTerminalExitRequest, WriteTextFileRequest,
};
#[allow(unused_imports)]
use agent_client_protocol::schema::{
    CreateTerminalResponse, KillTerminalResponse, ReleaseTerminalResponse, TerminalOutputResponse,
    WaitForTerminalExitResponse,
};
use agent_client_protocol::{Agent, Client, ConnectionTo, Error as AcpError};
use anyhow::{anyhow, Result};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::acp::fs::{self as acp_fs, FsSandbox};
use crate::acp::permissions::{self, PermissionPolicy};
use crate::acp::terminal::TerminalRegistry;
use crate::acp::transport::{spawn_stdio, SpawnedAgent};
use crate::acp::types::{map_session_update, AgentUpdate, AuthMethodInfo, StopReason};
use crate::config::AcpAgentStdioConfig;
use tokio::process::Child;

/// Channel capacity for the broadcast of [`AgentEvent`]s. Old events are
/// dropped if subscribers lag.
const EVENT_CHANNEL_CAP: usize = 256;

/// Outbound events surfaced by a running session.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A streaming session update (text chunk, tool call, plan, etc.).
    Update(AgentUpdate),
    /// The agent process emitted a fatal error. The session is shutting down.
    Fatal(String),
}

/// Result of a single prompt turn.
#[derive(Debug, Clone)]
pub struct PromptOutcome {
    pub stop_reason: StopReason,
}

/// Categorised session error. Distinguishes auth-required from other failures
/// so callers (UI) can render the auth-method picker inline.
#[derive(Debug)]
pub enum SessionError {
    AuthRequired { methods: Vec<AuthMethodInfo> },
    Other(anyhow::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::AuthRequired { .. } => write!(f, "agent requires authentication"),
            SessionError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SessionError::AuthRequired { .. } => None,
            SessionError::Other(e) => Some(e.as_ref()),
        }
    }
}

impl From<anyhow::Error> for SessionError {
    fn from(e: anyhow::Error) -> Self {
        SessionError::Other(e)
    }
}

impl SessionError {
    pub fn other<E: Into<anyhow::Error>>(e: E) -> Self {
        SessionError::Other(e.into())
    }
}

/// Handle to a running ACP session.
pub struct AgentSessionHandle {
    pub agent_name: String,
    /// Session id, present once `session/new` has succeeded. `None` while the
    /// agent still needs auth or before the first call to [`ensure_session`].
    session_id: Mutex<Option<SessionId>>,
    /// Auth methods advertised by the agent in the initialize response.
    pub auth_methods: Vec<AuthMethodInfo>,
    /// Whether the agent advertised `agent_capabilities.load_session` in
    /// its initialize response.
    pub supports_load_session: bool,
    /// MCP servers to forward into `session/new` and `session/load`,
    /// resolved at spawn time against the agent's `mcp_capabilities`.
    pub mcp_servers: Vec<McpServer>,
    /// Workspace root resolved at spawn time. Used to (re)create sessions.
    pub workspace_root: PathBuf,
    connection: ConnectionTo<Agent>,
    events: broadcast::Sender<AgentEvent>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    /// Joined on shutdown to surface the connection task's exit status.
    join: Mutex<Option<JoinHandle<Result<(), AcpError>>>>,
    /// Kept alive so the child is killed on drop.
    child: Mutex<Option<Child>>,
}

impl AgentSessionHandle {
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.events.subscribe()
    }

    /// Returns the current session id (None if `ensure_session` has not yet
    /// succeeded).
    pub async fn current_session_id(&self) -> Option<SessionId> {
        self.session_id.lock().await.clone()
    }

    /// Ensure a session exists. If the agent reports `auth_required`, returns
    /// [`SessionError::AuthRequired`] with the advertised methods so the UI
    /// can prompt the user.
    pub async fn ensure_session(&self) -> Result<SessionId, SessionError> {
        if let Some(id) = self.session_id.lock().await.clone() {
            return Ok(id);
        }
        let resp = self
            .connection
            .send_request(
                NewSessionRequest::new(self.workspace_root.clone())
                    .mcp_servers(self.mcp_servers.clone()),
            )
            .block_task()
            .await
            .map_err(|e| match e.code {
                ErrorCode::AuthRequired => SessionError::AuthRequired {
                    methods: self.auth_methods.clone(),
                },
                _ => SessionError::other(anyhow!("session/new failed: {e:?}")),
            })?;
        let id = resp.session_id.clone();
        *self.session_id.lock().await = Some(id.clone());
        Ok(id)
    }

    /// Attempt to load (resume) a previously created session by id.
    ///
    /// Only succeeds if the agent advertised `load_session` in its
    /// initialize response. On success, the handle's session id is set
    /// to the resumed id. Returns [`SessionError::AuthRequired`] if the
    /// agent demands authentication first.
    pub async fn load_session(
        &self,
        session_id: impl Into<SessionId>,
    ) -> Result<SessionId, SessionError> {
        if !self.supports_load_session {
            return Err(SessionError::other(anyhow!(
                "agent does not support session/load"
            )));
        }
        let id = session_id.into();
        self.connection
            .send_request(
                LoadSessionRequest::new(id.clone(), self.workspace_root.clone())
                    .mcp_servers(self.mcp_servers.clone()),
            )
            .block_task()
            .await
            .map_err(|e| match e.code {
                ErrorCode::AuthRequired => SessionError::AuthRequired {
                    methods: self.auth_methods.clone(),
                },
                _ => SessionError::other(anyhow!("session/load failed: {e:?}")),
            })?;
        *self.session_id.lock().await = Some(id.clone());
        Ok(id)
    }

    /// Send an `authenticate` request with the given method id.
    pub async fn authenticate(&self, method_id: impl Into<String>) -> Result<()> {
        let id = method_id.into();
        self.connection
            .send_request(AuthenticateRequest::new(id.clone()))
            .block_task()
            .await
            .map_err(|e| anyhow!("authenticate({id}) failed: {e:?}"))?;
        Ok(())
    }

    /// Send a plain text prompt to the agent.
    pub async fn prompt_text(&self, text: impl Into<String>) -> Result<PromptOutcome, SessionError> {
        self.prompt(vec![ContentBlock::Text(TextContent::new(text.into()))])
            .await
    }

    /// Send a fully-formed prompt to the agent. Will lazily create the
    /// session on first call; surfaces [`SessionError::AuthRequired`] if the
    /// agent rejects session creation.
    pub async fn prompt(&self, blocks: Vec<ContentBlock>) -> Result<PromptOutcome, SessionError> {
        let session_id = self.ensure_session().await?;
        let response = self
            .connection
            .send_request(PromptRequest::new(session_id, blocks))
            .block_task()
            .await
            .map_err(|e| SessionError::other(anyhow!("prompt failed: {e:?}")))?;
        Ok(PromptOutcome {
            stop_reason: response.stop_reason.into(),
        })
    }

    /// Send a `session/cancel` notification. No-op if no session yet.
    pub async fn cancel(&self) -> Result<()> {
        let id = match self.session_id.lock().await.clone() {
            Some(id) => id,
            None => return Ok(()),
        };
        self.connection
            .send_notification(CancelNotification::new(id))
            .map_err(|e| anyhow!("cancel failed: {e:?}"))?;
        Ok(())
    }

    /// Trigger an orderly shutdown.
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(tx) = self.shutdown.lock().await.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join.lock().await.take() {
            let _ = handle.await;
        }
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }
}

impl Drop for AgentSessionHandle {
    fn drop(&mut self) {
        // Best-effort: nudge the connection task to exit. The Mutexes here are
        // tokio Mutexes so we can't await; instead try_lock and rely on
        // `kill_on_drop(true)` from the spawn.
        if let Ok(mut g) = self.shutdown.try_lock() {
            if let Some(tx) = g.take() {
                let _ = tx.send(());
            }
        }
    }
}

/// Per-session shared state used by client-side request handlers.
struct SessionState {
    fs_sandbox: FsSandbox,
    terminals: Arc<TerminalRegistry>,
    permission_policy: PermissionPolicy,
    events: broadcast::Sender<AgentEvent>,
}

/// Spawn an agent and complete the initialize + new-session handshake.
pub async fn spawn_session(
    cfg: &AcpAgentStdioConfig,
    permission_policy: PermissionPolicy,
    mcp_configs: &[crate::config::McpConfig],
) -> Result<Arc<AgentSessionHandle>> {
    let SpawnedAgent {
        stdin,
        stdout,
        child,
    } = spawn_stdio(cfg)?;

    let transport = agent_client_protocol::ByteStreams::new(stdin.compat_write(), stdout.compat());

    let workspace_root = cfg
        .workspace_root
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow!("could not resolve a workspace root for agent"))?;

    let fs_sandbox = FsSandbox::new(Some(workspace_root.clone()));
    let terminals = Arc::new(TerminalRegistry::new());
    let (events_tx, _events_rx) = broadcast::channel(EVENT_CHANNEL_CAP);

    let state = Arc::new(SessionState {
        fs_sandbox,
        terminals: Arc::clone(&terminals),
        permission_policy,
        events: events_tx.clone(),
    });

    // We use a oneshot to ferry the cloned `ConnectionTo<Agent>` and the
    // initialize response (for `auth_methods`) out of the `connect_with`
    // closure. The closure then parks until shutdown.
    let (handshake_tx, handshake_rx) = oneshot::channel::<
        Result<(ConnectionTo<Agent>, InitializeResponse), AcpError>,
    >();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Build per-handler state clones up front. Each `on_receive_*` closure
    // takes ownership of its own clone.
    let state_for_notif = Arc::clone(&state);
    let state_for_perm = Arc::clone(&state);
    let state_for_read = Arc::clone(&state);
    let state_for_write = Arc::clone(&state);
    let state_for_term_create = Arc::clone(&state);
    let state_for_term_output = Arc::clone(&state);
    let state_for_term_release = Arc::clone(&state);
    let state_for_term_kill = Arc::clone(&state);
    let state_for_term_wait = Arc::clone(&state);

    let join: JoinHandle<Result<(), AcpError>> = tokio::spawn(async move {
        let result = Client
            .builder()
            .on_receive_notification(
                async move |notif: SessionNotification, _cx| {
                    let update = map_session_update(notif.update);
                    let _ = state_for_notif.events.send(AgentEvent::Update(update));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |req: RequestPermissionRequest, responder, _cx| {
                    let resp =
                        permissions::resolve_request(&req, &state_for_perm.permission_policy);
                    responder.respond(resp)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: ReadTextFileRequest, responder, _cx| {
                    match acp_fs::read_text_file(&state_for_read.fs_sandbox, req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: WriteTextFileRequest, responder, _cx| {
                    match acp_fs::write_text_file(&state_for_write.fs_sandbox, req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: CreateTerminalRequest, responder, _cx| {
                    match state_for_term_create.terminals.create(req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: TerminalOutputRequest, responder, _cx| {
                    match state_for_term_output.terminals.output(req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: ReleaseTerminalRequest, responder, _cx| {
                    match state_for_term_release.terminals.release(req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: KillTerminalRequest, responder, _cx| {
                    match state_for_term_kill.terminals.kill(req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request(
                async move |req: WaitForTerminalExitRequest, responder, _cx| {
                    match state_for_term_wait.terminals.wait_for_exit(req).await {
                        Ok(r) => responder.respond(r),
                        Err(e) => responder.respond_with_error(e),
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(transport, |connection: ConnectionTo<Agent>| async move {
                // Initialize, advertising fs + terminal capabilities.
                let caps = ClientCapabilities::new()
                    .fs(FileSystemCapabilities::new()
                        .read_text_file(true)
                        .write_text_file(true))
                    .terminal(true);
                let init = connection
                    .send_request(InitializeRequest::new(ProtocolVersion::V1).client_capabilities(caps))
                    .block_task()
                    .await?;

                // Hand the connection + init response back to the caller.
                // Session creation happens out-of-band so the caller can
                // observe `auth_required` errors.
                let _ = handshake_tx.send(Ok((connection.clone(), init)));

                // Wait for the controller to signal shutdown.
                let _ = shutdown_rx.await;
                Ok(())
            })
            .await;

        if let Err(ref e) = result {
            let _ = state.events.send(AgentEvent::Fatal(format!("{e:?}")));
        }
        result.map(|_| ())
    });

    // Wait for handshake to complete (or task to fail).
    let (connection, init) = match handshake_rx.await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            return Err(anyhow!("agent handshake failed: {e:?}"));
        }
        Err(_) => {
            // Task ended before handshake — surface its error if possible.
            let err_msg = match join.await {
                Ok(Ok(())) => "agent exited before handshake".to_string(),
                Ok(Err(e)) => format!("{e:?}"),
                Err(e) => format!("join error: {e}"),
            };
            return Err(anyhow!("agent handshake failed: {err_msg}"));
        }
    };

    let auth_methods: Vec<AuthMethodInfo> =
        init.auth_methods.iter().map(AuthMethodInfo::from).collect();
    let supports_load_session = init.agent_capabilities.load_session;
    let mcp_servers = crate::acp::mcp_passthrough::mcp_servers_from_configs(
        mcp_configs,
        &init.agent_capabilities.mcp_capabilities,
    );

    // Suppress unused-binding lint: `terminals` is a strong handle the
    // registry shares with each handler closure via `state`. Keeping the
    // outer binding alive ensures the registry's lifetime matches the
    // session even after handlers move their clones.
    let _ = &terminals;

    Ok(Arc::new(AgentSessionHandle {
        agent_name: cfg.name.clone(),
        session_id: Mutex::new(None),
        auth_methods,
        supports_load_session,
        mcp_servers,
        workspace_root,
        connection,
        events: events_tx,
        shutdown: Mutex::new(Some(shutdown_tx)),
        join: Mutex::new(Some(join)),
        child: Mutex::new(Some(child)),
    }))
}

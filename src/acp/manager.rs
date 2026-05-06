//! Global registry of running ACP agents.
//!
//! Mirrors the singleton pattern used by [`crate::mcp::ToolManager`].

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{anyhow, Result};

use crate::acp::permissions::PermissionPolicy;
use crate::acp::session::{spawn_session, AgentSessionHandle, SessionError};
use crate::acp::types::AuthMethodInfo;
use crate::config::{AcpAgentConfig, Config};

/// Error returned by [`AgentManager::ensure_session`].
pub enum EnsureSessionError {
    AuthRequired {
        handle: Arc<AgentSessionHandle>,
        methods: Vec<AuthMethodInfo>,
    },
    Other(anyhow::Error),
}

impl std::fmt::Debug for EnsureSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnsureSessionError::AuthRequired { methods, .. } => f
                .debug_struct("AuthRequired")
                .field("methods", methods)
                .finish_non_exhaustive(),
            EnsureSessionError::Other(e) => f.debug_tuple("Other").field(e).finish(),
        }
    }
}

impl std::fmt::Display for EnsureSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnsureSessionError::AuthRequired { .. } => write!(f, "agent requires authentication"),
            EnsureSessionError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for EnsureSessionError {}

#[derive(Default)]
pub struct AgentManager {
    sessions: Arc<RwLock<HashMap<String, Arc<AgentSessionHandle>>>>,
}

impl AgentManager {
    fn new() -> Self {
        Self::default()
    }

    /// Spawn a session for the given agent name (looked up in `Config`).
    /// If a session already exists, it is returned as-is.
    ///
    /// This only spawns the agent + completes the `initialize` handshake. It
    /// does NOT call `session/new`; use [`AgentSessionHandle::ensure_session`]
    /// for that so the caller can intercept `auth_required` errors.
    pub async fn ensure_started(&self, agent_name: &str) -> Result<Arc<AgentSessionHandle>> {
        if let Some(existing) = self
            .sessions
            .read()
            .map_err(|e| anyhow!(e.to_string()))?
            .get(agent_name)
            .cloned()
        {
            return Ok(existing);
        }

        let cfg_root = Config::default();
        let cfg = cfg_root
            .acp_agents
            .iter()
            .find(|a| a.name() == agent_name)
            .cloned()
            .ok_or_else(|| anyhow!("no ACP agent named '{}' is configured", agent_name))?;

        let handle = match cfg {
            AcpAgentConfig::Stdio(stdio) => {
                spawn_session(&stdio, PermissionPolicy::default(), &cfg_root.mcp_configs).await?
            }
        };

        self.sessions
            .write()
            .map_err(|e| anyhow!(e.to_string()))?
            .insert(agent_name.to_string(), Arc::clone(&handle));
        Ok(handle)
    }

    /// Convenience: spawn (if needed) and ensure a session is created. Returns
    /// the auth methods if the agent requires authentication first.
    pub async fn ensure_session(
        &self,
        agent_name: &str,
    ) -> Result<Arc<AgentSessionHandle>, EnsureSessionError> {
        let handle = self
            .ensure_started(agent_name)
            .await
            .map_err(EnsureSessionError::Other)?;
        match handle.ensure_session().await {
            Ok(_) => Ok(handle),
            Err(SessionError::AuthRequired { methods }) => Err(EnsureSessionError::AuthRequired {
                handle,
                methods,
            }),
            Err(SessionError::Other(e)) => Err(EnsureSessionError::Other(e)),
        }
    }

    pub fn get(&self, agent_name: &str) -> Result<Option<Arc<AgentSessionHandle>>> {
        Ok(self
            .sessions
            .read()
            .map_err(|e| anyhow!(e.to_string()))?
            .get(agent_name)
            .cloned())
    }

    pub fn list(&self) -> Result<Vec<String>> {
        Ok(self
            .sessions
            .read()
            .map_err(|e| anyhow!(e.to_string()))?
            .keys()
            .cloned()
            .collect())
    }

    pub async fn shutdown(&self, agent_name: &str) -> Result<()> {
        let handle = self
            .sessions
            .write()
            .map_err(|e| anyhow!(e.to_string()))?
            .remove(agent_name);
        if let Some(h) = handle {
            h.shutdown().await?;
        }
        Ok(())
    }

    pub async fn shutdown_all(&self) -> Result<()> {
        let drained: Vec<_> = {
            let mut g = self
                .sessions
                .write()
                .map_err(|e| anyhow!(e.to_string()))?;
            g.drain().collect()
        };
        for (_, handle) in drained {
            let _ = handle.shutdown().await;
        }
        Ok(())
    }
}

static AGENT_MANAGER: OnceLock<AgentManager> = OnceLock::new();

pub fn get_agent_manager() -> &'static AgentManager {
    AGENT_MANAGER.get_or_init(AgentManager::new)
}

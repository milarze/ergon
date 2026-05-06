//! Spawning external ACP agent processes over stdio.
//!
//! This mirrors the `yolo_one_shot_client.rs` pattern from the upstream
//! `agent-client-protocol` crate but parameterised for Ergon's
//! [`AcpAgentStdioConfig`](crate::config::AcpAgentStdioConfig).

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::config::AcpAgentStdioConfig;

/// Stdio handles plus the [`Child`] guard for a spawned agent.
pub struct SpawnedAgent {
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub child: Child,
}

/// Spawn the agent described by `cfg`. Stdin/stdout are piped; stderr inherits
/// the parent's so logs appear next to Ergon's own output.
pub fn spawn_stdio(cfg: &AcpAgentStdioConfig) -> Result<SpawnedAgent> {
    if cfg.command.trim().is_empty() {
        return Err(anyhow!("ACP agent '{}' has no command set", cfg.name));
    }

    let command_path = PathBuf::from(&cfg.command);
    let mut cmd = Command::new(&command_path);
    cmd.args(&cfg.args);
    for (name, value) in &cfg.env {
        cmd.env(name, value);
    }
    if let Some(root) = &cfg.workspace_root {
        let p = PathBuf::from(root);
        if p.is_dir() {
            cmd.current_dir(p);
        }
    }
    log::info!("ACP Agen command: {:?}", cmd);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn ACP agent '{}'", cfg.name))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("agent '{}' stdin was not piped", cfg.name))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("agent '{}' stdout was not piped", cfg.name))?;

    Ok(SpawnedAgent {
        stdin,
        stdout,
        child,
    })
}

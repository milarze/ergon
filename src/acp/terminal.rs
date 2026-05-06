//! Sandboxed terminal callbacks for ACP `terminal/*` requests.
//!
//! Each session owns a [`TerminalRegistry`]. When the agent asks the client
//! to spawn a process, we run it under tokio, stream its output into a
//! ring-buffered byte limit, and let the agent poll, wait, kill, and release
//! it.
//!
//! Phase 1 implements the spec faithfully but does **not** integrate with
//! Ergon's UI yet — output is captured in-memory only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use agent_client_protocol::schema::{
    CreateTerminalRequest, CreateTerminalResponse, KillTerminalRequest, KillTerminalResponse,
    ReleaseTerminalRequest, ReleaseTerminalResponse, TerminalExitStatus, TerminalId,
    TerminalOutputRequest, TerminalOutputResponse, WaitForTerminalExitRequest,
    WaitForTerminalExitResponse,
};
use agent_client_protocol::Error as AcpError;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::{watch, Mutex};

const DEFAULT_OUTPUT_BYTE_LIMIT: u64 = 1_048_576; // 1 MiB

/// State for a single running (or finished) terminal.
struct TerminalState {
    /// Captured stdout+stderr (interleaved). Truncated from the start when the
    /// length exceeds `byte_limit`.
    output: Arc<Mutex<Vec<u8>>>,
    /// Whether truncation has occurred.
    truncated: Arc<Mutex<bool>>,
    /// Byte limit for retained output.
    byte_limit: usize,
    /// `Some(status)` once the child has exited.
    exit_rx: watch::Receiver<Option<TerminalExitStatus>>,
    /// Used to send `kill()` to the child without taking ownership of it
    /// elsewhere. Cleared after the kill resolves.
    child: Arc<Mutex<Option<Child>>>,
}

#[derive(Default)]
pub struct TerminalRegistry {
    next_id: std::sync::atomic::AtomicU64,
    terminals: Mutex<HashMap<TerminalId, Arc<TerminalState>>>,
}

impl TerminalRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&self) -> TerminalId {
        let n = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        TerminalId::new(format!("term-{n}"))
    }

    pub async fn create(
        &self,
        request: CreateTerminalRequest,
    ) -> Result<CreateTerminalResponse, AcpError> {
        let mut cmd = Command::new(&request.command);
        cmd.args(&request.args);
        for v in &request.env {
            cmd.env(&v.name, &v.value);
        }
        if let Some(cwd) = &request.cwd {
            cmd.current_dir(PathBuf::from(cwd));
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            AcpError::internal_error().data(serde_json::Value::String(format!(
                "spawn failed: {e}"
            )))
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let byte_limit = request
            .output_byte_limit
            .unwrap_or(DEFAULT_OUTPUT_BYTE_LIMIT) as usize;
        let output = Arc::new(Mutex::new(Vec::<u8>::new()));
        let truncated = Arc::new(Mutex::new(false));
        let (exit_tx, exit_rx) = watch::channel(None);

        // Drain stdout
        if let Some(mut s) = stdout {
            let buf = Arc::clone(&output);
            let trunc = Arc::clone(&truncated);
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                loop {
                    match s.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => append_with_limit(&buf, &trunc, &chunk[..n], byte_limit).await,
                    }
                }
            });
        }
        // Drain stderr (interleaved)
        if let Some(mut s) = stderr {
            let buf = Arc::clone(&output);
            let trunc = Arc::clone(&truncated);
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                loop {
                    match s.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => append_with_limit(&buf, &trunc, &chunk[..n], byte_limit).await,
                    }
                }
            });
        }

        let id = self.alloc_id();
        let child_arc = Arc::new(Mutex::new(Some(child)));
        let state = Arc::new(TerminalState {
            output: Arc::clone(&output),
            truncated: Arc::clone(&truncated),
            byte_limit,
            exit_rx,
            child: Arc::clone(&child_arc),
        });

        // Spawn waiter that fills the exit watch
        let child_for_wait = Arc::clone(&child_arc);
        tokio::spawn(async move {
            // Take the child out of the slot so we own &mut it for `wait`.
            let mut guard = child_for_wait.lock().await;
            let Some(mut owned) = guard.take() else {
                let _ = exit_tx.send(Some(TerminalExitStatus::default()));
                return;
            };
            drop(guard); // release lock during wait
            let status = owned.wait().await;
            let exit = match status {
                Ok(s) => TerminalExitStatus::new()
                    .exit_code(s.code().map(|c| c as u32))
                    .signal(signal_name(&s)),
                Err(_) => TerminalExitStatus::default(),
            };
            let _ = exit_tx.send(Some(exit));
        });

        self.terminals.lock().await.insert(id.clone(), state);
        Ok(CreateTerminalResponse::new(id))
    }

    pub async fn output(
        &self,
        request: TerminalOutputRequest,
    ) -> Result<TerminalOutputResponse, AcpError> {
        let state = self.get(&request.terminal_id).await?;
        let bytes = state.output.lock().await.clone();
        let truncated = *state.truncated.lock().await;
        let exit = state.exit_rx.borrow().clone();
        let mut resp =
            TerminalOutputResponse::new(String::from_utf8_lossy(&bytes).to_string(), truncated);
        if let Some(e) = exit {
            resp = resp.exit_status(e);
        }
        Ok(resp)
    }

    pub async fn release(
        &self,
        request: ReleaseTerminalRequest,
    ) -> Result<ReleaseTerminalResponse, AcpError> {
        let state = self.terminals.lock().await.remove(&request.terminal_id);
        if let Some(state) = state {
            // Best-effort kill if still running.
            if state.exit_rx.borrow().is_none() {
                if let Some(mut child) = state.child.lock().await.take() {
                    let _ = child.kill().await;
                }
            }
        }
        Ok(ReleaseTerminalResponse::default())
    }

    pub async fn kill(
        &self,
        request: KillTerminalRequest,
    ) -> Result<KillTerminalResponse, AcpError> {
        let state = self.get(&request.terminal_id).await?;
        if let Some(mut child) = state.child.lock().await.take() {
            let _ = child.kill().await;
        }
        Ok(KillTerminalResponse::default())
    }

    pub async fn wait_for_exit(
        &self,
        request: WaitForTerminalExitRequest,
    ) -> Result<WaitForTerminalExitResponse, AcpError> {
        let state = self.get(&request.terminal_id).await?;
        let mut rx = state.exit_rx.clone();
        loop {
            if let Some(exit) = rx.borrow().clone() {
                return Ok(WaitForTerminalExitResponse::new(exit));
            }
            if rx.changed().await.is_err() {
                return Err(AcpError::internal_error()
                    .data(serde_json::Value::String("exit channel closed".into())));
            }
        }
    }

    async fn get(&self, id: &TerminalId) -> Result<Arc<TerminalState>, AcpError> {
        self.terminals
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| {
                AcpError::invalid_params().data(serde_json::Value::String(format!(
                    "unknown terminal id: {}",
                    id.0
                )))
            })
    }

    /// Drop all terminals (kill any still running). Called on session shutdown.
    pub async fn shutdown(&self) {
        let drained: Vec<_> = self.terminals.lock().await.drain().collect();
        for (_id, state) in drained {
            if let Some(mut child) = state.child.lock().await.take() {
                let _ = child.kill().await;
            }
            // Touch byte_limit so the field isn't unused on platforms without signals.
            let _ = state.byte_limit;
        }
    }
}

async fn append_with_limit(
    buf: &Arc<Mutex<Vec<u8>>>,
    truncated: &Arc<Mutex<bool>>,
    data: &[u8],
    limit: usize,
) {
    let mut g = buf.lock().await;
    g.extend_from_slice(data);
    if g.len() > limit {
        let drop_n = g.len() - limit;
        // Truncate at a UTF-8 char boundary if possible.
        let mut start = drop_n;
        while start < g.len() && (g[start] & 0b1100_0000) == 0b1000_0000 {
            start += 1;
        }
        g.drain(..start);
        *truncated.lock().await = true;
    }
}

#[cfg(unix)]
fn signal_name(status: &std::process::ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|s| s.to_string())
}

#[cfg(not(unix))]
fn signal_name(_status: &std::process::ExitStatus) -> Option<String> {
    None
}

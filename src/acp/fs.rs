//! Sandboxed filesystem callbacks for ACP `fs/*` requests.
//!
//! Each session is bound to an optional `workspace_root`. All paths the agent
//! requests must resolve to a path that is inside that root after canonicalisation.
//! `None` means "no sandbox" — only enable that for trusted, local agents.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::{
    ReadTextFileRequest, ReadTextFileResponse, WriteTextFileRequest, WriteTextFileResponse,
};
use agent_client_protocol::Error as AcpError;

#[derive(Debug, Clone, Default)]
pub struct FsSandbox {
    pub root: Option<PathBuf>,
}

impl FsSandbox {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    fn check_path(&self, path: &Path) -> Result<(), AcpError> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        // Walk through ancestors looking for an existing prefix we can canonicalise.
        // The leaf may not exist yet (write case).
        let mut probe: PathBuf = path.to_path_buf();
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
        loop {
            if let Ok(canon) = probe.canonicalize() {
                if canon.starts_with(&canonical_root) {
                    return Ok(());
                }
                break;
            }
            if !probe.pop() {
                break;
            }
        }
        Err(AcpError::invalid_params().data(serde_json::json!({
            "reason": "path outside workspace root",
            "path": path.display().to_string(),
            "workspace_root": root.display().to_string(),
        })))
    }
}

/// Handle `fs/read_text_file`.
pub async fn read_text_file(
    sandbox: &FsSandbox,
    request: ReadTextFileRequest,
) -> Result<ReadTextFileResponse, AcpError> {
    sandbox.check_path(&request.path)?;
    let content = tokio::fs::read_to_string(&request.path)
        .await
        .map_err(|e| AcpError::internal_error().data(serde_json::Value::String(e.to_string())))?;

    let sliced = slice_lines(&content, request.line, request.limit);
    Ok(ReadTextFileResponse::new(sliced))
}

/// Handle `fs/write_text_file`.
pub async fn write_text_file(
    sandbox: &FsSandbox,
    request: WriteTextFileRequest,
) -> Result<WriteTextFileResponse, AcpError> {
    sandbox.check_path(&request.path)?;
    if let Some(parent) = request.path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AcpError::internal_error().data(serde_json::Value::String(e.to_string())))?;
        }
    }
    tokio::fs::write(&request.path, &request.content)
        .await
        .map_err(|e| AcpError::internal_error().data(serde_json::Value::String(e.to_string())))?;
    Ok(WriteTextFileResponse::new())
}

fn slice_lines(content: &str, line: Option<u32>, limit: Option<u32>) -> String {
    if line.is_none() && limit.is_none() {
        return content.to_string();
    }
    let start = line.unwrap_or(1).saturating_sub(1) as usize;
    let take = limit.map(|l| l as usize).unwrap_or(usize::MAX);
    content
        .lines()
        .skip(start)
        .take(take)
        .collect::<Vec<_>>()
        .join("\n")
}

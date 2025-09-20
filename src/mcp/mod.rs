use crate::config::McpConfig;
use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
    RmcpError,
};
use tokio::process::Command;

#[allow(dead_code)]
pub async fn init(_config: McpConfig) -> Result<(), RmcpError> {
    let client = ()
        .serve(
            TokioChildProcess::new(Command::new("uvx").configure(|cmd| {
                cmd.arg("mcp-server-git");
            }))
            .map_err(RmcpError::transport_creation::<TokioChildProcess>)?,
        )
        .await?;
    let server_info = client.peer_info();
    log::info!("Connected to server: {:?}", server_info);
    let tools = client.list_tools(Default::default()).await?;
    log::info!("Available tools: {:?}", tools);
    let tool_result = client
        .call_tool(CallToolRequestParam {
            name: "git_status".into(),
            arguments: serde_json::json!({ "repo_path": "." }).as_object().cloned(),
        })
        .await?;
    log::info!("Tool result: {:?}", tool_result);
    client.cancel().await?;
    Ok(())
}

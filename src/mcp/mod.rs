use crate::config::McpConfig;
use anyhow::Result;
use rmcp::{
    service::{RunningService, ServiceExt},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, ConfigureCommandExt,
        StreamableHttpClientTransport, TokioChildProcess,
    },
    RoleClient,
};
use tokio::process::Command;

pub type McpClient = RunningService<RoleClient, ()>;

#[allow(dead_code)]
pub async fn init(config: McpConfig) -> Result<RunningService<RoleClient, ()>> {
    let client = match config {
        McpConfig::Stdio(cfg) => {
            let transport = TokioChildProcess::new(Command::new(cfg.command).configure(|cmd| {
                cmd.args(cfg.args);
            }))?;
            ().serve(transport).await?
        }
        McpConfig::StreamableHttp(server_config) => {
            let transport = StreamableHttpClientTransport::from_uri(server_config.endpoint);
            ().serve(transport).await?
        }
    };
    Ok(client)
}

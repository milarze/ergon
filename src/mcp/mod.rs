use std::sync::{Arc, RwLock};

use crate::config::McpConfig;
use anyhow::Result;
use iced::futures::future::join_all;
use rmcp::{
    service::{RunningService, ServiceExt},
    transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess},
    RoleClient,
};
use tokio::process::Command;

pub type McpClient = RunningService<RoleClient, ()>;

#[derive(Debug)]
pub struct ToolManager {
    mcp_clients: Arc<RwLock<Vec<McpClient>>>,
    tools: Arc<RwLock<Vec<crate::models::Tool>>>,
}

impl ToolManager {
    fn new() -> Self {
        Self {
            mcp_clients: Arc::new(RwLock::new(Vec::new())),
            tools: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn load_tools(&self) -> Result<()> {
        let clients: Vec<McpClient> = join_all(
            crate::config::Config::default()
                .mcp_configs
                .iter()
                .map(async |config| init(config.clone()).await),
        )
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        let mut all_tools: Vec<crate::models::Tool> = Vec::new();
        for client in clients.iter() {
            let response: Vec<crate::models::Tool> = client
                .list_all_tools()
                .await?
                .into_iter()
                .map(<rmcp::model::Tool as Into<crate::models::Tool>>::into)
                .collect();
            all_tools.extend(response);
        }

        {
            let mut mcpclients = self
                .mcp_clients
                .write()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            *mcpclients = clients;
        }

        {
            let mut tools_lock = self
                .tools
                .write()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            *tools_lock = all_tools.into_iter().collect();
        }
        Ok(())
    }

    pub fn get_tools(&self) -> Result<Vec<crate::models::Tool>> {
        let tools_lock = self
            .tools
            .read()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(tools_lock.clone())
    }
}

pub async fn init(config: McpConfig) -> Result<McpClient> {
    log::info!("Initializing MCP client with config: {:?}", config);
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

static TOOL_MANAGER: std::sync::OnceLock<ToolManager> = std::sync::OnceLock::new();

pub fn get_tool_manager() -> &'static ToolManager {
    TOOL_MANAGER.get_or_init(ToolManager::new)
}

impl From<rmcp::model::Tool> for crate::models::Tool {
    fn from(tool: rmcp::model::Tool) -> Self {
        crate::models::Tool::Function(crate::models::Function {
            name: tool.name.to_string(),
            description: tool.description.unwrap_or_default().to_string(),
            parameters: serde_json::Value::Object((*tool.input_schema).clone()),
        })
    }
}

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

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
    /// Map of MCP client name to MCP client instance
    mcp_clients: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
    /// List of all available tools
    /// Each tool's name is prefixed with the MCP client name to ensure uniqueness
    tools: Arc<RwLock<Vec<crate::models::Tool>>>,
}

impl ToolManager {
    fn new() -> Self {
        Self {
            mcp_clients: Arc::new(RwLock::new(HashMap::new())),
            tools: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn load_tools(&self) -> Result<()> {
        let clients: HashMap<String, Arc<McpClient>> = join_all(
            crate::config::Config::default()
                .mcp_configs
                .iter()
                .map(async |config| (config.name().to_string(), init(config.clone()).await)),
        )
        .await
        .into_iter()
        .map(|(name, result)| result.map(|client| (name, Arc::new(client))))
        .collect::<Result<HashMap<String, Arc<McpClient>>, _>>()?;

        let mut all_tools: Vec<crate::models::Tool> = Vec::new();
        for (client_name, client) in clients.iter() {
            let response: Vec<crate::models::Tool> = client
                .list_all_tools()
                .await?
                .into_iter()
                .map(|tool| {
                    let mut tool = tool.into();
                    match &mut tool {
                        crate::models::Tool::Function(func) => {
                            func.name = format!("__{}__{}", client_name, func.name);
                        }
                    };
                    tool
                })
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

    pub fn get_client_by_tool_call(&self, tool_call_name: &str) -> Result<Option<Arc<McpClient>>> {
        let parts: Vec<&str> = tool_call_name.splitn(2, "__").collect();
        if parts.len() != 2 {
            return Ok(None);
        }
        let client_name = parts[0];

        let mcpclients = self
            .mcp_clients
            .read()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if let Some(client) = mcpclients.get(client_name) {
            Ok(Some(client.to_owned()))
        } else {
            Ok(None)
        }
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

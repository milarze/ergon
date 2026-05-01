pub mod auth;
pub mod oauth_callback;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::config::{McpAuthConfig, McpConfig};
use anyhow::Result;
use iced::futures::future::join_all;
use rmcp::{
    service::{RunningService, ServiceExt},
    transport::{
        auth::{AuthClient, AuthorizationManager},
        streamable_http_client::StreamableHttpClientTransportConfig,
        ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess,
    },
    RoleClient,
};
use tokio::process::Command;

use self::auth::FileCredentialStore;

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
        .filter_map(|(name, result)| match result {
            Ok(client) => Some((name, Arc::new(client))),
            Err(e) => {
                log::error!(
                    "Failed to initialize MCP client '{}': {}. Skipping this server.",
                    name,
                    e
                );
                None
            }
        })
        .collect::<HashMap<String, Arc<McpClient>>>();

        let mut all_tools: Vec<crate::models::Tool> = Vec::new();
        for (client_name, client) in clients.iter() {
            match client.list_all_tools().await {
                Ok(tools) => {
                    let response: Vec<crate::models::Tool> = tools
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
                Err(e) => {
                    log::error!(
                        "Failed to list tools for MCP client '{}': {}. Skipping.",
                        client_name,
                        e
                    );
                }
            }
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
        let (client_name, tool_name) =
            match self.tool_client_and_name_by_tool_call(tool_call_name.to_string())? {
                Some((client_name, tool_name)) => (client_name, tool_name),
                None => {
                    return Ok(None);
                }
            };

        log::info!(
            "Looking for MCP client '{}' for tool call '{}'",
            client_name,
            tool_name
        );

        let mcpclients = self
            .mcp_clients
            .read()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if let Some(client) = mcpclients.get(&client_name) {
            Ok(Some(client.to_owned()))
        } else {
            Ok(None)
        }
    }

    pub fn tool_client_and_name_by_tool_call(
        &self,
        tool_call_name: String,
    ) -> Result<Option<(String, String)>> {
        let parts: Vec<&str> = tool_call_name
            .strip_prefix("__")
            .unwrap_or(&tool_call_name)
            .splitn(2, "__")
            .collect();
        if parts.len() != 2 {
            return Ok(None);
        }
        let client_name = parts[0];
        let tool_name = parts[1].to_string();

        log::info!(
            "Looking for MCP client '{}' for tool call '{}'",
            client_name,
            tool_name
        );

        Ok(Some((client_name.to_string(), tool_name)))
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
            init_streamable_http(
                &server_config.name,
                &server_config.endpoint,
                &server_config.auth,
            )
            .await?
        }
    };
    Ok(client)
}

/// Initialize a StreamableHTTP MCP client with the appropriate auth configuration.
async fn init_streamable_http(
    server_name: &str,
    endpoint: &str,
    auth_config: &McpAuthConfig,
) -> Result<McpClient> {
    match auth_config {
        McpAuthConfig::None => {
            log::info!(
                "MCP '{}': connecting to {} with no authentication",
                server_name,
                endpoint
            );
            let transport = StreamableHttpClientTransport::from_uri(endpoint);
            let client = ().serve(transport).await?;
            Ok(client)
        }

        McpAuthConfig::BearerToken { token } => {
            log::info!(
                "MCP '{}': connecting to {} with bearer token authentication",
                server_name,
                endpoint
            );
            let config =
                StreamableHttpClientTransportConfig::with_uri(endpoint).auth_header(token.clone());
            let transport = StreamableHttpClientTransport::from_config(config);
            let client = ().serve(transport).await?;
            Ok(client)
        }

        McpAuthConfig::OAuth2 { .. } => {
            log::info!(
                "MCP '{}': connecting to {} with OAuth2 authentication",
                server_name,
                endpoint
            );

            let mut auth_manager = AuthorizationManager::new(endpoint)
                .await
                .map_err(|e| anyhow::anyhow!("OAuth2 manager creation failed: {}", e))?;

            // Set file-backed credential store for persistence
            auth_manager.set_credential_store(FileCredentialStore::new(server_name));

            // Startup path: only use stored credentials. Interactive authorization
            // is triggered explicitly from the Settings UI via `auth::run_oauth_authorization`.
            let has_stored = auth_manager
                .initialize_from_store()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to load stored credentials: {}", e))?;

            if !has_stored {
                return Err(anyhow::anyhow!(
                    "MCP '{}' requires OAuth2 authorization. Open Settings and click 'Authenticate' to sign in.",
                    server_name
                ));
            }

            log::info!("MCP '{}': using stored OAuth2 credentials", server_name);

            // Create AuthClient that wraps reqwest::Client with automatic token injection
            let auth_client = AuthClient::new(reqwest::Client::default(), auth_manager);

            let config = StreamableHttpClientTransportConfig::with_uri(endpoint);
            let transport = StreamableHttpClientTransport::with_client(auth_client, config);
            let client = ().serve(transport).await?;
            Ok(client)
        }
    }
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

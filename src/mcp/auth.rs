use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use async_trait::async_trait;
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationSession, CredentialStore, StoredCredentials,
};
use tokio::sync::RwLock;

use crate::config::{Config, McpAuthConfig, McpStreamableHttpConfig, StoredOAuthTokens};
use crate::mcp::oauth_callback;

/// A credential store backed by the Ergon settings.json file.
///
/// Stores and loads OAuth2 credentials for a specific MCP server,
/// keyed by the server name in the `oauth_tokens` section of config.
pub struct FileCredentialStore {
    server_name: String,
    /// In-memory cache so we don't hit the filesystem on every token read.
    cache: Arc<RwLock<Option<StoredCredentials>>>,
}

impl FileCredentialStore {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Convert our internal StoredOAuthTokens to rmcp's StoredCredentials.
    ///
    /// Both types are Serialize/Deserialize, and StoredCredentials is
    /// #[non_exhaustive], so we go through JSON to construct it.
    fn to_stored_credentials(tokens: &StoredOAuthTokens) -> Option<StoredCredentials> {
        // Build a JSON object matching StoredCredentials' serde layout:
        // { client_id, token_response, granted_scopes, token_received_at }
        //
        // token_response is an OAuthTokenResponse (oauth2 StandardTokenResponse),
        // which expects: { access_token, token_type, [refresh_token], [expires_in], [scope] }
        let mut token_response = serde_json::Map::new();
        token_response.insert(
            "access_token".to_string(),
            serde_json::Value::String(tokens.access_token.clone()),
        );
        token_response.insert(
            "token_type".to_string(),
            serde_json::Value::String("bearer".to_string()),
        );
        if let Some(ref refresh) = tokens.refresh_token {
            token_response.insert(
                "refresh_token".to_string(),
                serde_json::Value::String(refresh.clone()),
            );
        }
        // Compute remaining lifetime for expires_in
        if let Some(expires_at) = tokens.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let remaining = expires_at.saturating_sub(now);
            token_response.insert(
                "expires_in".to_string(),
                serde_json::Value::Number(serde_json::Number::from(remaining)),
            );
        }
        if !tokens.granted_scopes.is_empty() {
            token_response.insert(
                "scope".to_string(),
                serde_json::Value::String(tokens.granted_scopes.join(" ")),
            );
        }

        // Build the outer StoredCredentials JSON
        let mut creds_json = serde_json::Map::new();
        creds_json.insert(
            "client_id".to_string(),
            serde_json::Value::String(tokens.client_id.clone()),
        );
        creds_json.insert(
            "token_response".to_string(),
            serde_json::Value::Object(token_response),
        );
        creds_json.insert(
            "granted_scopes".to_string(),
            serde_json::Value::Array(
                tokens
                    .granted_scopes
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
        // Compute token_received_at: approximate as now (the important thing is
        // that the auth manager checks remaining time via expires_in)
        if let Some(expires_at) = tokens.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            creds_json.insert(
                "token_received_at".to_string(),
                serde_json::Value::Number(serde_json::Number::from(now.min(expires_at))),
            );
        }

        serde_json::from_value(serde_json::Value::Object(creds_json)).ok()
    }

    /// Convert rmcp's StoredCredentials to our StoredOAuthTokens for persistence.
    ///
    /// We serialize to JSON and extract the fields we need, avoiding direct
    /// access to oauth2 crate types.
    fn from_stored_credentials(creds: &StoredCredentials) -> Option<StoredOAuthTokens> {
        // Serialize the entire StoredCredentials to JSON
        let creds_json = serde_json::to_value(creds).ok()?;
        let creds_obj = creds_json.as_object()?;

        let client_id = creds_obj.get("client_id")?.as_str()?.to_string();

        let token_response = creds_obj.get("token_response")?.as_object()?;

        let access_token = token_response.get("access_token")?.as_str()?.to_string();

        let refresh_token = token_response
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expires_at = token_response
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .map(|dur| now + dur);

        let granted_scopes = creds_obj
            .get("granted_scopes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Some(StoredOAuthTokens {
            client_id,
            access_token,
            refresh_token,
            expires_at,
            granted_scopes,
        })
    }

    fn load_from_file(&self) -> Option<StoredOAuthTokens> {
        let config = Config::default();
        config.oauth_tokens.get(&self.server_name).cloned()
    }

    fn save_to_file(&self, tokens: StoredOAuthTokens) -> Result<(), AuthError> {
        let mut config = Config::default();
        config.oauth_tokens.insert(self.server_name.clone(), tokens);
        config.update_settings();
        Ok(())
    }

    fn clear_from_file(&self) -> Result<(), AuthError> {
        let mut config = Config::default();
        config.oauth_tokens.remove(&self.server_name);
        config.update_settings();
        Ok(())
    }
}

#[async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        // Check in-memory cache first
        {
            let cache = self.cache.read().await;
            if cache.is_some() {
                return Ok(cache.clone());
            }
        }

        // Load from file
        let stored_tokens = self.load_from_file();
        let creds = stored_tokens.and_then(|t| Self::to_stored_credentials(&t));

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = creds.clone();
        }

        Ok(creds)
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        // Persist to file
        if let Some(tokens) = Self::from_stored_credentials(&credentials) {
            self.save_to_file(tokens)?;
        }

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(credentials);
        }

        Ok(())
    }

    async fn clear(&self) -> Result<(), AuthError> {
        // Clear from file
        self.clear_from_file()?;

        // Clear cache
        {
            let mut cache = self.cache.write().await;
            *cache = None;
        }

        Ok(())
    }
}

/// Run the interactive OAuth2 authorization flow for the given server config.
///
/// Performs metadata discovery, dynamic client registration, opens the user's
/// browser at the authorization URL, listens on a local callback server,
/// and exchanges the authorization code for tokens.
///
/// On success, the tokens are persisted via [`FileCredentialStore`] so a
/// subsequent startup will use them automatically.
pub async fn run_oauth_authorization(
    server_config: McpStreamableHttpConfig,
) -> std::result::Result<(), String> {
    run_oauth_authorization_inner(server_config)
        .await
        .map_err(|e| format!("{}", e))
}

async fn run_oauth_authorization_inner(server_config: McpStreamableHttpConfig) -> Result<()> {
    let (scopes, client_name, redirect_port) = match &server_config.auth {
        McpAuthConfig::OAuth2 {
            scopes,
            client_name,
            redirect_port,
        } => (scopes.clone(), client_name.clone(), *redirect_port),
        _ => {
            anyhow::bail!(
                "Server '{}' is not configured for OAuth2 authentication",
                server_config.name
            );
        }
    };

    let server_name = server_config.name.clone();
    let endpoint = server_config.endpoint.clone();

    log::info!(
        "MCP '{}': starting interactive OAuth2 authorization against {}",
        server_name,
        endpoint
    );

    let mut auth_manager = AuthorizationManager::new(&endpoint)
        .await
        .map_err(|e| anyhow::anyhow!("OAuth2 manager creation failed: {}", e))?;

    auth_manager.set_credential_store(FileCredentialStore::new(&server_name));

    // Discover OAuth2 metadata
    let metadata = auth_manager
        .discover_metadata()
        .await
        .map_err(|e| anyhow::anyhow!("OAuth2 metadata discovery failed: {}", e))?;
    auth_manager.set_metadata(metadata);

    let redirect_uri = format!("http://127.0.0.1:{}/callback", redirect_port);

    let scope_refs: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
    let selected_scopes = auth_manager.select_scopes(None, &scope_refs);
    let scope_strs: Vec<&str> = selected_scopes.iter().map(|s| s.as_str()).collect();

    // Create authorization session (handles dynamic client registration)
    let session = AuthorizationSession::new(
        auth_manager,
        &scope_strs,
        &redirect_uri,
        Some(client_name.as_str()),
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("OAuth2 authorization session failed: {}", e))?;

    let auth_url = session.get_authorization_url().to_string();

    // Start the callback listener first so the port is bound before the browser
    // redirects back. tokio::spawn eagerly polls the future.
    let port = redirect_port;
    let callback_task = tokio::spawn(async move { oauth_callback::wait_for_oauth_callback(port).await });

    log::info!(
        "MCP '{}': authorization URL: {}",
        server_name,
        auth_url
    );

    // Open the user's browser; log (but don't fail) if it can't be opened.
    if let Err(e) = open::that(&auth_url) {
        log::error!(
            "MCP '{}': failed to open browser ({}). Please manually visit: {}",
            server_name,
            e,
            auth_url
        );
    } else {
        log::info!("MCP '{}': opened browser for OAuth2 authorization", server_name);
    }

    let callback_result = callback_task
        .await
        .map_err(|e| anyhow::anyhow!("OAuth callback task failed: {}", e))??;

    session
        .handle_callback(&callback_result.code, &callback_result.state)
        .await
        .map_err(|e| anyhow::anyhow!("OAuth2 token exchange failed: {}", e))?;

    log::info!(
        "MCP '{}': OAuth2 authorization completed successfully",
        server_name
    );

    Ok(())
}

/// Clear stored OAuth2 credentials for a given server name.
pub async fn clear_oauth_tokens(server_name: String) -> std::result::Result<(), String> {
    let store = FileCredentialStore::new(&server_name);
    store.clear().await.map_err(|e| format!("{}", e))
}

use std::collections::HashMap;
use std::fmt::Display;

use iced::Theme;

use serde::{ser::SerializeStruct, Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub endpoint: String,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            endpoint: "https://api.openai.com/v1/".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub endpoint: String,
    pub max_tokens: u32,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            endpoint: "https://api.anthropic.com/v1/".to_string(),
            max_tokens: 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VllmConfig {
    pub endpoint: String,
    pub model: String,
}

impl Default for VllmConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://localhost:8000/v1/".to_string(),
            model: "google/gemma-3-270m".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpStdioConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

fn default_client_name() -> String {
    "Ergon".to_string()
}
fn default_redirect_port() -> u16 {
    8585
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum McpAuthConfig {
    #[default]
    None,
    BearerToken {
        token: String,
    },
    OAuth2 {
        #[serde(default)]
        scopes: Vec<String>,
        #[serde(default = "default_client_name")]
        client_name: String,
        #[serde(default = "default_redirect_port")]
        redirect_port: u16,
    },
}

impl Display for McpAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpAuthConfig::None => write!(f, "None"),
            McpAuthConfig::BearerToken { .. } => write!(f, "Bearer Token"),
            McpAuthConfig::OAuth2 { .. } => write!(f, "OAuth2"),
        }
    }
}

/// Stored OAuth2 tokens for persistence between app restarts
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOAuthTokens {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub granted_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpStreamableHttpConfig {
    pub name: String,
    pub endpoint: String,
    #[serde(default)]
    pub auth: McpAuthConfig,
}

impl Default for McpStreamableHttpConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            endpoint: String::new(),
            auth: McpAuthConfig::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpConfig {
    Stdio(McpStdioConfig),
    StreamableHttp(McpStreamableHttpConfig),
}

/// Configuration for an external ACP agent (Stdio transport).
///
/// ACP agents are separate processes that own their own LLM credentials and
/// provider logic. Ergon spawns them and speaks ACP over stdio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AcpAgentStdioConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Literal env vars to inject when spawning the agent.
    #[serde(default)]
    pub env: Vec<(String, String)>,
    /// Optional sandbox root for filesystem operations the agent requests.
    /// `None` means the directory in which Ergon was launched.
    #[serde(default)]
    pub workspace_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcpAgentConfig {
    Stdio(AcpAgentStdioConfig),
}

impl Default for AcpAgentConfig {
    fn default() -> Self {
        AcpAgentConfig::Stdio(AcpAgentStdioConfig {
            name: "default-acp-agent".to_string(),
            command: String::new(),
            args: vec![],
            env: vec![],
            workspace_root: None,
        })
    }
}

impl Display for AcpAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpAgentConfig::Stdio(_) => write!(f, "Stdio: {}", self.name()),
        }
    }
}

impl AcpAgentConfig {
    pub fn name(&self) -> &str {
        match self {
            AcpAgentConfig::Stdio(cfg) => &cfg.name,
        }
    }

    pub fn validate_name(&self) -> bool {
        let name = self.name();
        !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    pub fn set_name(&mut self, new_name: String) {
        match self {
            AcpAgentConfig::Stdio(cfg) => cfg.name = new_name,
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        McpConfig::Stdio(McpStdioConfig {
            name: "default-stdio-mcp".to_string(),
            command: "".to_string(),
            args: vec![],
        })
    }
}

impl Display for McpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpConfig::Stdio(_) => write!(f, "Stdio: {}", self.name()),
            McpConfig::StreamableHttp(_) => write!(f, "StreamableHttp: {}", self.name()),
        }
    }
}

impl McpConfig {
    pub fn name(&self) -> &str {
        match self {
            McpConfig::Stdio(cfg) => &cfg.name,
            McpConfig::StreamableHttp(cfg) => &cfg.name,
        }
    }

    pub fn validate_name(&self) -> bool {
        let name = self.name();
        name.matches(r"^[a-zA-Z0-9_\-]+$").count() == 1
    }

    pub fn set_name(&mut self, new_name: String) {
        match self {
            McpConfig::Stdio(cfg) => cfg.name = new_name,
            McpConfig::StreamableHttp(cfg) => cfg.name = new_name,
        }
    }
}

/// Persisted resumable-session state for an ACP agent.
///
/// Stored per agent name, written when a session id is first allocated and
/// cleared on explicit "forget" / failed resume. Keyed by the user-supplied
/// agent name in [`AcpAgentConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAcpSession {
    /// The most recent session id we held for this agent.
    pub session_id: String,
    /// The workspace root that session was created against. Used to gate
    /// resume so we don't load a session into a different cwd.
    pub workspace_root: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
    pub openai: OpenAIConfig,
    pub anthropic: AnthropicConfig,
    pub vllm: VllmConfig,
    pub mcp_configs: Vec<McpConfig>,
    pub acp_agents: Vec<AcpAgentConfig>,
    pub acp_session_state: HashMap<String, StoredAcpSession>,
    pub oauth_tokens: HashMap<String, StoredOAuthTokens>,
    pub settings_file: String,
}

impl Config {
    fn load_settings(path: Option<String>) -> Self {
        let settings_file_path = path.unwrap_or_else(Self::settings_file_path);
        if std::fs::exists(&settings_file_path).is_err() {
            let default_settings = Self::fresh(settings_file_path.clone());
            let settings_json = serde_json::to_string(&default_settings).unwrap();
            std::fs::write(&settings_file_path, settings_json)
                .expect("Failed to write default settings");
            return default_settings;
        }

        if let Ok(settings_json) = std::fs::read_to_string(&settings_file_path) {
            if let Ok(settings) = serde_json::from_str::<Self>(&settings_json) {
                settings
            } else {
                Self::fresh(settings_file_path)
            }
        } else {
            Self::fresh(settings_file_path)
        }
    }

    fn fresh(settings_file: String) -> Self {
        Self {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![McpConfig::default()],
            acp_agents: vec![],
            acp_session_state: HashMap::new(),
            oauth_tokens: HashMap::new(),
            settings_file,
        }
    }

    pub fn update_settings(&self) {
        let settings_json = serde_json::to_string(self).expect("Failed to serialize settings");
        std::fs::write(&self.settings_file, settings_json).expect("Failed to write settings file");
    }

    fn settings_file_path() -> String {
        let settings_dir = home::home_dir()
            .map(|path| path.join(".ergon"))
            .unwrap_or_else(|| ".ergon".into());

        if !settings_dir.exists() {
            std::fs::create_dir_all(&settings_dir).expect("Failed to create settings directory");
        }

        settings_dir
            .join(SETTINGS_FILE)
            .to_string_lossy()
            .into_owned()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::load_settings(None)
    }
}

impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let theme_name = match self.theme {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            _ => "Default",
        };
        let mut state = serializer.serialize_struct("Config", 7)?;
        state.serialize_field("theme", theme_name)?;
        state.serialize_field("openai", &self.openai)?;
        state.serialize_field("anthropic", &self.anthropic)?;
        state.serialize_field("vllm", &self.vllm)?;
        state.serialize_field("mcp", &self.mcp_configs)?;
        if !self.acp_agents.is_empty() {
            state.serialize_field("acp", &self.acp_agents)?;
        }
        if !self.acp_session_state.is_empty() {
            state.serialize_field("acp_session_state", &self.acp_session_state)?;
        }
        if !self.oauth_tokens.is_empty() {
            state.serialize_field("oauth_tokens", &self.oauth_tokens)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Fields {
            Theme,
            OpenAI,
            Anthropic,
            Vllm,
            McpConfigs,
            AcpAgents,
            AcpSessionState,
            OAuthTokens,
            Other,
        }

        impl<'de> Deserialize<'de> for Fields {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldsVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldsVisitor {
                    type Value = Fields;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("a field name")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        Ok(match value {
                            "theme" => Fields::Theme,
                            "openai" => Fields::OpenAI,
                            "anthropic" => Fields::Anthropic,
                            "vllm" => Fields::Vllm,
                            "mcp" => Fields::McpConfigs,
                            "acp" => Fields::AcpAgents,
                            "acp_session_state" => Fields::AcpSessionState,
                            "oauth_tokens" => Fields::OAuthTokens,
                            _ => Fields::Other,
                        })
                    }
                }

                deserializer.deserialize_identifier(FieldsVisitor)
            }
        }

        struct ConfigVisitor;
        impl<'de> serde::de::Visitor<'de> for ConfigVisitor {
            type Value = Config;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a configuration object")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut theme = None;
                let mut openai = None;
                let mut anthropic = None;
                let mut vllm = None;
                let mut mcp_configs = None;
                let mut acp_agents = None;
                let mut acp_session_state = None;
                let mut oauth_tokens = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Fields::Theme => {
                            if theme.is_some() {
                                return Err(serde::de::Error::duplicate_field("theme"));
                            }
                            let theme_name: &str = map.next_value()?;
                            theme = Some(match theme_name {
                                "Light" => Theme::Light,
                                "Dark" => Theme::Dark,
                                _ => Theme::Dark,
                            });
                        }
                        Fields::OpenAI => {
                            let openai_map =
                                map.next_value::<serde_json::Map<String, serde_json::Value>>()?;
                            openai = Some(
                                OpenAIConfig::deserialize(serde_json::Value::Object(openai_map))
                                    .map_err(serde::de::Error::custom)?,
                            );
                        }
                        Fields::Anthropic => {
                            let anthropic_map =
                                map.next_value::<serde_json::Map<String, serde_json::Value>>()?;
                            anthropic = Some(
                                AnthropicConfig::deserialize(serde_json::Value::Object(
                                    anthropic_map,
                                ))
                                .map_err(serde::de::Error::custom)?,
                            );
                        }
                        Fields::Vllm => {
                            let vllm_map =
                                map.next_value::<serde_json::Map<String, serde_json::Value>>()?;
                            vllm = Some(
                                VllmConfig::deserialize(serde_json::Value::Object(vllm_map))
                                    .map_err(serde::de::Error::custom)?,
                            );
                        }
                        Fields::McpConfigs => {
                            let mcp_configs_vec = map.next_value::<Vec<serde_json::Value>>()?;
                            let mut configs = Vec::new();
                            for mcp_value in mcp_configs_vec {
                                let mcp_config = McpConfig::deserialize(mcp_value)
                                    .map_err(serde::de::Error::custom)?;
                                configs.push(mcp_config);
                            }
                            mcp_configs = Some(configs);
                        }
                        Fields::AcpAgents => {
                            let acp_vec = map.next_value::<Vec<serde_json::Value>>()?;
                            let mut agents = Vec::new();
                            for v in acp_vec {
                                let agent = AcpAgentConfig::deserialize(v)
                                    .map_err(serde::de::Error::custom)?;
                                agents.push(agent);
                            }
                            acp_agents = Some(agents);
                        }
                        Fields::AcpSessionState => {
                            let m = map
                                .next_value::<HashMap<String, StoredAcpSession>>()?;
                            acp_session_state = Some(m);
                        }
                        Fields::OAuthTokens => {
                            let tokens_map =
                                map.next_value::<HashMap<String, StoredOAuthTokens>>()?;
                            oauth_tokens = Some(tokens_map);
                        }
                        Fields::Other => {
                            // Ignore unknown fields for forward compatibility.
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let theme = theme.ok_or_else(|| serde::de::Error::missing_field("theme"))?;
                let openai = openai.unwrap_or_default();
                let anthropic = anthropic.unwrap_or_default();
                let vllm = vllm.unwrap_or_default();
                let mcp_configs = mcp_configs.unwrap_or_default();
                let acp_agents = acp_agents.unwrap_or_default();
                let acp_session_state = acp_session_state.unwrap_or_default();
                let oauth_tokens = oauth_tokens.unwrap_or_default();
                Ok(Config {
                    theme,
                    openai,
                    anthropic,
                    vllm,
                    mcp_configs,
                    acp_agents,
                    acp_session_state,
                    oauth_tokens,
                    settings_file: Config::settings_file_path(),
                })
            }
        }

        deserializer.deserialize_struct("Config", &["theme"], ConfigVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, Theme::Dark);
    }

    #[test]
    fn test_serialize_config() {
        let config = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![McpConfig::default()],
            acp_agents: vec![],
            acp_session_state: HashMap::new(),
            oauth_tokens: HashMap::new(),
            settings_file: "./test.json".to_string(),
        };
        let serialized = serde_json::to_string(&config).unwrap();
        assert!(serialized.contains("\"theme\":\"Dark\""));
        assert!(serialized
            .contains("\"openai\":{\"api_key\":\"\",\"endpoint\":\"https://api.openai.com/v1/\"}"));
        assert!(serialized.contains(
            "\"anthropic\":{\"api_key\":\"\",\"endpoint\":\"https://api.anthropic.com/v1/\",\"max_tokens\":1024}"
        ));
        assert!(serialized.contains(
            "\"vllm\":{\"endpoint\":\"https://localhost:8000/v1/\",\"model\":\"google/gemma-3-270m\"}"
        ));
        assert!(serialized.contains(
            "\"mcp\":[{\"Stdio\":{\"name\":\"default-stdio-mcp\",\"command\":\"\",\"args\":[]}}]"
        ));
    }

    #[test]
    fn test_deserialize_config() {
        let json =
            r#"{"theme":"Light","openai":{"api_key":"","endpoint":"https://api.openai.com/v1/"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Light);
        assert_eq!(config.openai.api_key, "");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
    }

    #[test]
    fn test_deserialize_config_without_anthropic() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
    }

    #[test]
    fn test_deserialize_config_with_anthropic() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
        assert_eq!(config.anthropic.max_tokens, 1024);
    }

    #[test]
    fn test_deserialize_config_without_openai() {
        let json = r#"{"theme":"Dark","anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
    }

    #[test]
    fn test_deserialize_config_with_vllm() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024},"vllm":{"endpoint":"https://vllm.cluster.local/v1/","model":"google/gemma-3-270m"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
        assert_eq!(config.anthropic.max_tokens, 1024);
        assert_eq!(config.vllm.endpoint, "https://vllm.cluster.local/v1/");
        assert_eq!(config.vllm.model, "google/gemma-3-270m");
    }

    #[test]
    fn test_deserialize_config_without_vllm() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
        assert_eq!(config.anthropic.max_tokens, 1024);
        assert_eq!(config.vllm.endpoint, "https://localhost:8000/v1/");
        assert_eq!(config.vllm.model, "google/gemma-3-270m");
    }

    #[test]
    fn test_deserialize_config_with_mcp() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024},"vllm":{"endpoint":"https://vllm.cluster.local/v1/","model":"google/gemma-3-270m"},"mcp":[{"Stdio":{"name":"stdio-mcp","command":"python3","args":["-u","mcp_stdio.py"]}},{"StreamableHttp":{"name":"http-mcp","endpoint":"http://localhost:9000/v1/"}}]}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
        assert_eq!(config.anthropic.max_tokens, 1024);
        assert_eq!(config.vllm.endpoint, "https://vllm.cluster.local/v1/");
        assert_eq!(config.vllm.model, "google/gemma-3-270m");
        assert_eq!(config.mcp_configs.len(), 2);
        match &config.mcp_configs[0] {
            McpConfig::Stdio(stdio_config) => {
                assert_eq!(stdio_config.name, "stdio-mcp");
                assert_eq!(stdio_config.command, "python3");
                assert_eq!(stdio_config.args, vec!["-u", "mcp_stdio.py"]);
            }
            _ => panic!("Expected Stdio config"),
        }
        match &config.mcp_configs[1] {
            McpConfig::StreamableHttp(http_config) => {
                assert_eq!(http_config.name, "http-mcp");
                assert_eq!(http_config.endpoint, "http://localhost:9000/v1/");
            }
            _ => panic!("Expected StreamableHttp config"),
        }
    }

    #[test]
    fn test_deserialize_streamable_http_without_auth_defaults_to_none() {
        // Existing configs without an `auth` field should deserialize with McpAuthConfig::None
        let json =
            r#"{"StreamableHttp":{"name":"test-http","endpoint":"http://localhost:9000/v1/"}}"#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        if let McpConfig::StreamableHttp(http_config) = &config {
            assert_eq!(http_config.name, "test-http");
            assert_eq!(http_config.endpoint, "http://localhost:9000/v1/");
            assert_eq!(http_config.auth, McpAuthConfig::None);
        } else {
            panic!("Expected StreamableHttp config");
        }
    }

    #[test]
    fn test_roundtrip_streamable_http_auth_none() {
        let config = McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "test".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            auth: McpAuthConfig::None,
        });
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_roundtrip_streamable_http_auth_bearer() {
        let config = McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "test".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            auth: McpAuthConfig::BearerToken {
                token: "sk-my-secret-token".to_string(),
            },
        });
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
        // Verify the token is in the JSON
        assert!(json.contains("sk-my-secret-token"));
    }

    #[test]
    fn test_roundtrip_streamable_http_auth_oauth2() {
        let config = McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "test".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            auth: McpAuthConfig::OAuth2 {
                scopes: vec!["read".to_string(), "write".to_string()],
                client_name: "MyApp".to_string(),
                redirect_port: 9090,
            },
        });
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_roundtrip_streamable_http_auth_oauth2_defaults() {
        // OAuth2 with default values should roundtrip correctly
        let config = McpConfig::StreamableHttp(McpStreamableHttpConfig {
            name: "test".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            auth: McpAuthConfig::OAuth2 {
                scopes: Vec::new(),
                client_name: "Ergon".to_string(),
                redirect_port: 8585,
            },
        });
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_roundtrip_config_with_oauth_tokens() {
        let mut oauth_tokens = HashMap::new();
        oauth_tokens.insert(
            "test-server".to_string(),
            StoredOAuthTokens {
                client_id: "client-123".to_string(),
                access_token: "access-xyz".to_string(),
                refresh_token: Some("refresh-abc".to_string()),
                expires_at: Some(1700000000),
                granted_scopes: vec!["read".to_string()],
            },
        );
        let config = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![],
            acp_agents: vec![],
            acp_session_state: HashMap::new(),
            oauth_tokens,
            settings_file: "./test.json".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.oauth_tokens, deserialized.oauth_tokens);
        let stored = deserialized.oauth_tokens.get("test-server").unwrap();
        assert_eq!(stored.client_id, "client-123");
        assert_eq!(stored.access_token, "access-xyz");
        assert_eq!(stored.refresh_token, Some("refresh-abc".to_string()));
        assert_eq!(stored.expires_at, Some(1700000000));
        assert_eq!(stored.granted_scopes, vec!["read".to_string()]);
    }

    #[test]
    fn test_deserialize_config_without_oauth_tokens() {
        // Configs without oauth_tokens field should have empty HashMap
        let json = r#"{"theme":"Dark"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.oauth_tokens.is_empty());
        assert!(config.acp_session_state.is_empty());
    }

    #[test]
    fn test_roundtrip_config_with_acp_session_state() {
        let mut acp_session_state = HashMap::new();
        acp_session_state.insert(
            "my-agent".to_string(),
            StoredAcpSession {
                session_id: "sess-abcdef".to_string(),
                workspace_root: "/home/me/project".to_string(),
            },
        );
        let config = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![],
            acp_agents: vec![],
            acp_session_state,
            oauth_tokens: HashMap::new(),
            settings_file: "./test.json".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("acp_session_state"));
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.acp_session_state, deserialized.acp_session_state);
        let stored = deserialized.acp_session_state.get("my-agent").unwrap();
        assert_eq!(stored.session_id, "sess-abcdef");
        assert_eq!(stored.workspace_root, "/home/me/project");
    }

    #[test]
    fn test_deserialize_config_without_mcp() {
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024},"vllm":{"endpoint":"https://vllm.cluster.local/v1/","model":"google/gemma-3-270m"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.theme, Theme::Dark);
        assert_eq!(config.openai.api_key, "test_key");
        assert_eq!(config.openai.endpoint, "https://api.openai.com/v1/");
        assert_eq!(config.anthropic.api_key, "test_anthropic_key");
        assert_eq!(config.anthropic.endpoint, "https://api.anthropic.com/v1/");
        assert_eq!(config.anthropic.max_tokens, 1024);
        assert_eq!(config.vllm.endpoint, "https://vllm.cluster.local/v1/");
        assert_eq!(config.vllm.model, "google/gemma-3-270m");
        assert!(config.mcp_configs.is_empty());
    }
}

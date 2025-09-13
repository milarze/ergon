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
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct McpStreamableHttpConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpConfig {
    Stdio(McpStdioConfig),
    StreamableHttp(McpStreamableHttpConfig),
}

impl Default for McpConfig {
    fn default() -> Self {
        McpConfig::Stdio(McpStdioConfig {
            command: "".to_string(),
            args: vec![],
        })
    }
}

impl Display for McpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpConfig::Stdio(_) => write!(f, "Stdio"),
            McpConfig::StreamableHttp(_) => write!(f, "StreamableHttp"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
    pub openai: OpenAIConfig,
    pub anthropic: AnthropicConfig,
    pub vllm: VllmConfig,
    pub mcp_configs: Vec<McpConfig>,
    pub settings_file: String,
}

impl Config {
    fn load_settings(path: Option<String>) -> Self {
        let settings_file_path = path.unwrap_or_else(Self::settings_file_path);
        if std::fs::exists(&settings_file_path).is_err() {
            let default_settings = Self {
                theme: Theme::default(),
                openai: OpenAIConfig::default(),
                anthropic: AnthropicConfig::default(),
                vllm: VllmConfig::default(),
                mcp_configs: vec![McpConfig::default()],
                settings_file: settings_file_path.clone(),
            };
            let settings_json = serde_json::to_string(&default_settings).unwrap();
            std::fs::write(&settings_file_path, settings_json)
                .expect("Failed to write default settings");
            return default_settings;
        }

        if let Ok(settings_json) = std::fs::read_to_string(&settings_file_path) {
            if let Ok(settings) = serde_json::from_str::<Self>(&settings_json) {
                settings
            } else {
                Self {
                    theme: Theme::default(),
                    openai: OpenAIConfig::default(),
                    anthropic: AnthropicConfig::default(),
                    vllm: VllmConfig::default(),
                    mcp_configs: vec![McpConfig::default()],
                    settings_file: settings_file_path.clone(),
                }
            }
        } else {
            Self {
                theme: Theme::default(),
                openai: OpenAIConfig::default(),
                anthropic: AnthropicConfig::default(),
                vllm: VllmConfig::default(),
                mcp_configs: vec![McpConfig::default()],
                settings_file: settings_file_path.clone(),
            }
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
        let mut state = serializer.serialize_struct("Config", 1)?;
        state.serialize_field("theme", theme_name)?;
        state.serialize_field("openai", &self.openai)?;
        state.serialize_field("anthropic", &self.anthropic)?;
        state.serialize_field("vllm", &self.vllm)?;
        state.serialize_field("mcp", &self.mcp_configs)?;
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
                        match value {
                            "theme" => Ok(Fields::Theme),
                            "openai" => Ok(Fields::OpenAI),
                            "anthropic" => Ok(Fields::Anthropic),
                            "vllm" => Ok(Fields::Vllm),
                            "mcp" => Ok(Fields::McpConfigs),
                            _ => Err(E::unknown_field(value, &["theme", "openai"])),
                        }
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
                                _ => Theme::default(),
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
                    }
                }

                let theme = theme.ok_or_else(|| serde::de::Error::missing_field("theme"))?;
                let openai = openai.unwrap_or_default();
                let anthropic = anthropic.unwrap_or_default();
                let vllm = vllm.unwrap_or_default();
                let mcp_configs = mcp_configs.unwrap_or_default();
                Ok(Config {
                    theme,
                    openai,
                    anthropic,
                    vllm,
                    mcp_configs,
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
        assert_eq!(config.theme, Theme::default());
    }

    #[test]
    fn test_serialize_config() {
        let config = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![McpConfig::default()],
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
        assert!(serialized.contains("\"mcp\":[{\"Stdio\":{\"command\":\"\",\"args\":[]}}]"));
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
        let json = r#"{"theme":"Dark","openai":{"api_key":"test_key","endpoint":"https://api.openai.com/v1/"},"anthropic":{"api_key":"test_anthropic_key","endpoint":"https://api.anthropic.com/v1/","max_tokens":1024},"vllm":{"endpoint":"https://vllm.cluster.local/v1/","model":"google/gemma-3-270m"},"mcp":[{"Stdio":{"command":"python3","args":["-u","mcp_stdio.py"]}},{"StreamableHttp":{"endpoint":"http://localhost:9000/v1/"}}]}"#;
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
                assert_eq!(stdio_config.command, "python3");
                assert_eq!(stdio_config.args, vec!["-u", "mcp_stdio.py"]);
            }
            _ => panic!("Expected Stdio config"),
        }
        match &config.mcp_configs[1] {
            McpConfig::StreamableHttp(http_config) => {
                assert_eq!(http_config.endpoint, "http://localhost:9000/v1/");
            }
            _ => panic!("Expected StreamableHttp config"),
        }
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

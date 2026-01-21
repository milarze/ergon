use iced::widget::{button, column, container, pick_list, row, text, text_input, Text};
use iced::{Alignment, Element, Length, Theme};
use iced_aw::number_input;

use crate::config::{Config, McpConfig, McpStdioConfig, McpStreamableHttpConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigType {
    Stdio,
    StreamableHttp,
}

impl std::fmt::Display for McpConfigType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpConfigType::Stdio => write!(f, "Stdio"),
            McpConfigType::StreamableHttp => write!(f, "Streamable HTTP"),
        }
    }
}

impl McpConfigType {
    const ALL: [McpConfigType; 2] = [McpConfigType::Stdio, McpConfigType::StreamableHttp];
}

#[derive(Debug, Clone, Default)]
pub struct State {
    // Required to be public for dynamically changing the theme
    pub config: Config,
}

#[derive(Debug, Clone)]
pub enum SettingsAction {
    ChangeTheme(Theme),
    ChangeOpenAIKey(String),
    ChangeOpenAIUrl(String),
    ChangeAnthropicKey(String),
    ChangeAnthropicUrl(String),
    ChangeAnthropicMaxTokens(u32),
    ChangeVllmUrl(String),
    ChangeVllmModel(String),
    AddMcpConfig,
    ChangeMcpConfigName(usize, String),
    ChangeMcpConfigType(usize, bool), // index, true for Stdio, false for StreamableHttp
    ChangeMcpStdioCommand(usize, String),
    ChangeMcpStdioArgs(usize, String), // comma-separated args string
    ChangeMcpHttpEndpoint(usize, String),
    RemoveMcpConfig(usize),
    SaveSettings,
}

impl State {
    pub fn update(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::ChangeTheme(theme) => {
                self.config.theme = theme;
            }
            SettingsAction::ChangeOpenAIKey(api_key) => {
                self.config.openai.api_key = api_key;
            }
            SettingsAction::ChangeOpenAIUrl(endpoint) => {
                self.config.openai.endpoint = endpoint;
            }
            SettingsAction::ChangeAnthropicKey(api_key) => {
                self.config.anthropic.api_key = api_key;
            }
            SettingsAction::ChangeAnthropicUrl(endpoint) => {
                self.config.anthropic.endpoint = endpoint;
            }
            SettingsAction::ChangeAnthropicMaxTokens(max_tokens) => {
                self.config.anthropic.max_tokens = max_tokens;
            }
            SettingsAction::ChangeVllmUrl(endpoint) => {
                self.config.vllm.endpoint = endpoint;
            }
            SettingsAction::ChangeVllmModel(model) => {
                self.config.vllm.model = model;
            }
            SettingsAction::AddMcpConfig => {
                self.config.mcp_configs.push(McpConfig::default());
            }
            SettingsAction::ChangeMcpConfigName(index, name) => {
                if let Some(config) = self.config.mcp_configs.get_mut(index) {
                    config.set_name(name);
                }
            }
            SettingsAction::ChangeMcpConfigType(index, is_stdio) => {
                if let Some(config) = self.config.mcp_configs.get_mut(index) {
                    *config = if is_stdio {
                        McpConfig::Stdio(McpStdioConfig::default())
                    } else {
                        McpConfig::StreamableHttp(McpStreamableHttpConfig::default())
                    };
                }
            }
            SettingsAction::ChangeMcpStdioCommand(index, command) => {
                if let Some(McpConfig::Stdio(stdio_config)) = self.config.mcp_configs.get_mut(index)
                {
                    stdio_config.command = command;
                }
            }
            SettingsAction::ChangeMcpStdioArgs(index, args_str) => {
                if let Some(McpConfig::Stdio(stdio_config)) = self.config.mcp_configs.get_mut(index)
                {
                    stdio_config.args = args_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            SettingsAction::ChangeMcpHttpEndpoint(index, endpoint) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    http_config.endpoint = endpoint;
                }
            }
            SettingsAction::RemoveMcpConfig(index) => {
                if index < self.config.mcp_configs.len() {
                    self.config.mcp_configs.remove(index);
                }
            }
            SettingsAction::SaveSettings => {
                self.config.update_settings();
            }
        }
    }

    pub fn view(&self) -> Element<'_, SettingsAction> {
        let col = column![
            self.theme_view(),
            self.openai_view(),
            self.anthropic_view(),
            self.vllm_view(),
            self.mcp_configs_view(),
            button("Save Settings").on_press(SettingsAction::SaveSettings)
        ]
        .spacing(20)
        .padding(20)
        .align_x(Alignment::Center);
        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    fn theme_view(&self) -> iced::widget::Row<'_, SettingsAction> {
        row![
            button("Light").on_press(SettingsAction::ChangeTheme(Theme::Light)),
            button("Dark").on_press(SettingsAction::ChangeTheme(Theme::Dark)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn openai_view(&self) -> iced::widget::Row<'_, SettingsAction> {
        row![
            text("OpenAI API Key:"),
            text_input("Enter API Key", &self.config.openai.api_key)
                .on_input(SettingsAction::ChangeOpenAIKey),
            text("Endpoint:"),
            text_input("Enter Endpoint", &self.config.openai.endpoint)
                .on_input(SettingsAction::ChangeOpenAIUrl),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn anthropic_view(&self) -> iced::widget::Row<'_, SettingsAction> {
        row![
            text("Anthropic API Key:"),
            text_input("Enter API Key", &self.config.anthropic.api_key)
                .on_input(SettingsAction::ChangeAnthropicKey),
            text("Endpoint:"),
            text_input("Enter Endpoint", &self.config.anthropic.endpoint)
                .on_input(SettingsAction::ChangeAnthropicUrl),
            text("Max Tokens:"),
            number_input(&self.config.anthropic.max_tokens, 1..=4096, |value| {
                SettingsAction::ChangeAnthropicMaxTokens(value)
            })
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn vllm_view(&self) -> iced::widget::Row<'_, SettingsAction> {
        row![
            text("vLLM Endpoint:"),
            text_input("Enter Endpoint", &self.config.vllm.endpoint)
                .on_input(SettingsAction::ChangeVllmUrl),
            text("Model:"),
            text_input("Enter Model", &self.config.vllm.model)
                .on_input(SettingsAction::ChangeVllmModel),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn mcp_configs_view(&self) -> iced::widget::Column<'_, SettingsAction> {
        let mut column = column![text("MCP Servers:").size(18)];

        for (index, mcp_config) in self.config.mcp_configs.iter().enumerate() {
            let config_type = match mcp_config {
                McpConfig::Stdio(_) => McpConfigType::Stdio,
                McpConfig::StreamableHttp(_) => McpConfigType::StreamableHttp,
            };

            let type_picker = pick_list(
                &McpConfigType::ALL[..],
                Some(config_type),
                move |selected_type| {
                    SettingsAction::ChangeMcpConfigType(
                        index,
                        matches!(selected_type, McpConfigType::Stdio),
                    )
                },
            );

            let config_fields = match mcp_config {
                McpConfig::Stdio(stdio_config) => {
                    let args_str = stdio_config.args.join(", ");
                    row![
                        text("Command:"),
                        text_input("Enter command", &stdio_config.command)
                            .on_input(move |cmd| SettingsAction::ChangeMcpStdioCommand(index, cmd)),
                        text("Args:"),
                        text_input("comma,separated,args", &args_str)
                            .on_input(move |args| SettingsAction::ChangeMcpStdioArgs(index, args)),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                }
                McpConfig::StreamableHttp(http_config) => row![
                    text("Endpoint:"),
                    text_input("Enter endpoint URL", &http_config.endpoint).on_input(
                        move |endpoint| SettingsAction::ChangeMcpHttpEndpoint(index, endpoint)
                    ),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            };

            column = column.push(
                row![
                    text_input("Name", mcp_config.name())
                        .on_input(move |name| { SettingsAction::ChangeMcpConfigName(index, name) }),
                    type_picker,
                    config_fields,
                    button(iced_fonts::lucide::trash())
                        .on_press(SettingsAction::RemoveMcpConfig(index))
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            );
        }

        column
            .push(button(iced_fonts::lucide::plus()).on_press(SettingsAction::AddMcpConfig))
            .spacing(10)
            .align_x(Alignment::Center)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{AnthropicConfig, OpenAIConfig, VllmConfig};

    use super::*;

    #[test]
    fn test_update_theme() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeTheme(Theme::Dark));
        assert_eq!(state.config.theme, Theme::Dark);
    }

    #[test]
    fn test_update_openai_key() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeOpenAIKey("new_api_key".to_string()));
        assert_eq!(state.config.openai.api_key, "new_api_key");
    }

    #[test]
    fn test_update_openai_url() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeOpenAIUrl(
            "https://new.endpoint.com".to_string(),
        ));
        assert_eq!(state.config.openai.endpoint, "https://new.endpoint.com");
    }

    #[test]
    fn test_update_anthropic_key() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeAnthropicKey(
            "new_anthropic_key".to_string(),
        ));
        assert_eq!(state.config.anthropic.api_key, "new_anthropic_key");
    }

    #[test]
    fn test_update_anthropic_url() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeAnthropicUrl(
            "https://new.anthropic.endpoint.com".to_string(),
        ));
        assert_eq!(
            state.config.anthropic.endpoint,
            "https://new.anthropic.endpoint.com"
        );
    }

    #[test]
    fn test_update_anthropic_max_tokens() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeAnthropicMaxTokens(2048));
        assert_eq!(state.config.anthropic.max_tokens, 2048);
    }

    #[test]
    fn test_update_vllm_url() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeVllmUrl(
            "http://new.vllm.endpoint.com".to_string(),
        ));
        assert_eq!(state.config.vllm.endpoint, "http://new.vllm.endpoint.com");
    }

    #[test]
    fn test_update_vllm_model() {
        let mut state = State::default();
        state.update(SettingsAction::ChangeVllmModel("new-model".to_string()));
        assert_eq!(state.config.vllm.model, "new-model");
    }

    #[test]
    fn test_save_settings() {
        let mut state = State {
            config: Config {
                theme: Theme::Light,
                openai: OpenAIConfig {
                    api_key: String::new(),
                    endpoint: "https://api.openai.com/v1/".to_string(),
                },
                anthropic: AnthropicConfig {
                    api_key: String::new(),
                    endpoint: "https://api.anthropic.com/v1/".to_string(),
                    max_tokens: 1024,
                },
                vllm: VllmConfig {
                    endpoint: "http://localhost:8000/v1/".to_string(),
                    model: "google/gemma-3-270m".to_string(),
                },
                mcp_configs: vec![],
                settings_file: "./test.json".to_string(),
            },
        };
        state.update(SettingsAction::ChangeTheme(Theme::Dark));
        state.update(SettingsAction::ChangeOpenAIKey("test_key".to_string()));
        state.update(SettingsAction::ChangeOpenAIUrl(
            "https://api.test.com".to_string(),
        ));
        state.update(SettingsAction::ChangeAnthropicKey("hello".to_string()));
        state.update(SettingsAction::ChangeAnthropicUrl(
            "https://api.anthropic.com/v1/".to_string(),
        ));
        state.update(SettingsAction::SaveSettings);
        state.update(SettingsAction::AddMcpConfig);

        // Assuming update_settings persists the changes, we can check the config
        assert_eq!(state.config.theme, Theme::Dark);
        assert_eq!(state.config.openai.api_key, "test_key");
        assert_eq!(state.config.openai.endpoint, "https://api.test.com");

        assert_eq!(state.config.anthropic.api_key, "hello");
        assert_eq!(
            state.config.anthropic.endpoint,
            "https://api.anthropic.com/v1/"
        );
        assert_eq!(state.config.anthropic.max_tokens, 1024);
        assert_eq!(state.config.vllm.endpoint, "http://localhost:8000/v1/");
        assert_eq!(state.config.vllm.model, "google/gemma-3-270m");
        assert_eq!(state.config.mcp_configs.len(), 1);
    }
}

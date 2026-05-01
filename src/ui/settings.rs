use std::collections::HashMap;

use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Task, Theme};
use iced_aw::number_input;

use crate::config::{Config, McpAuthConfig, McpConfig, McpStdioConfig, McpStreamableHttpConfig};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAuthType {
    None,
    BearerToken,
    OAuth2,
}

impl std::fmt::Display for McpAuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpAuthType::None => write!(f, "None"),
            McpAuthType::BearerToken => write!(f, "Bearer Token"),
            McpAuthType::OAuth2 => write!(f, "OAuth2"),
        }
    }
}

impl McpAuthType {
    const ALL: [McpAuthType; 3] = [
        McpAuthType::None,
        McpAuthType::BearerToken,
        McpAuthType::OAuth2,
    ];
}

impl From<&McpAuthConfig> for McpAuthType {
    fn from(config: &McpAuthConfig) -> Self {
        match config {
            McpAuthConfig::None => McpAuthType::None,
            McpAuthConfig::BearerToken { .. } => McpAuthType::BearerToken,
            McpAuthConfig::OAuth2 { .. } => McpAuthType::OAuth2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum AuthStatus {
    #[default]
    Idle,
    InProgress,
    Error(String),
    JustAuthenticated,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    // Required to be public for dynamically changing the theme
    pub config: Config,
    /// Snapshot of the last-persisted config. Used to detect changes on save
    /// and to decide whether OAuth buttons should be enabled for a given row
    /// (only configs that match what's on disk can be authenticated).
    saved_config: Config,
    /// OAuth auth status keyed by server name (stable across add/remove/reorder).
    auth_status: HashMap<String, AuthStatus>,
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
    ChangeMcpHttpAuthType(usize, McpAuthType),
    ChangeMcpHttpBearerToken(usize, String),
    ChangeMcpHttpOAuthScopes(usize, String),
    ChangeMcpHttpOAuthClientName(usize, String),
    ChangeMcpHttpOAuthRedirectPort(usize, u16),
    RemoveMcpConfig(usize),
    SaveSettings,
    /// Emitted after `SaveSettings` completes. Consumed by the app shell to
    /// trigger reloading of models and/or tools if the relevant configs changed.
    SaveCompleted {
        llm_changed: bool,
        mcp_changed: bool,
    },
    StartOAuthAuth(usize),
    OAuthAuthFinished(String, Result<(), String>),
    ClearOAuthTokens(usize),
    OAuthTokensCleared(String, Result<(), String>),
}

impl State {
    /// Create a new settings state. Initializes both the editable `config` and
    /// the `saved_config` baseline from the on-disk settings file.
    pub fn new() -> Self {
        let config = Config::default();
        Self {
            saved_config: config.clone(),
            config,
            auth_status: HashMap::new(),
        }
    }

    /// Returns true if any LLM provider config changed between `old` and `new`.
    fn llm_configs_changed(old: &Config, new: &Config) -> bool {
        old.openai != new.openai || old.anthropic != new.anthropic || old.vllm != new.vllm
    }

    /// Returns true if the MCP server list changed.
    fn mcp_configs_changed(old: &Config, new: &Config) -> bool {
        old.mcp_configs != new.mcp_configs
    }

    /// Look up the saved (on-disk) version of the MCP config at the given index
    /// in the draft list. Returns Some only if a saved config with the same name
    /// exists *and* its OAuth2 settings match the draft — i.e. there are no
    /// unsaved edits that would make interactive auth meaningless.
    fn saved_matching_http_config(&self, index: usize) -> Option<&McpStreamableHttpConfig> {
        let draft = self.config.mcp_configs.get(index)?;
        let draft_http = match draft {
            McpConfig::StreamableHttp(c) => c,
            _ => return None,
        };
        if draft_http.name.is_empty() {
            return None;
        }
        for saved in &self.saved_config.mcp_configs {
            if let McpConfig::StreamableHttp(saved_http) = saved {
                if saved_http.name == draft_http.name && saved_http == draft_http {
                    return Some(saved_http);
                }
            }
        }
        None
    }

    pub fn update(&mut self, action: SettingsAction) -> Task<SettingsAction> {
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
            SettingsAction::ChangeMcpHttpAuthType(index, auth_type) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    http_config.auth = match auth_type {
                        McpAuthType::None => McpAuthConfig::None,
                        McpAuthType::BearerToken => McpAuthConfig::BearerToken {
                            token: String::new(),
                        },
                        McpAuthType::OAuth2 => McpAuthConfig::OAuth2 {
                            scopes: Vec::new(),
                            client_name: "Ergon".to_string(),
                            redirect_port: 8585,
                        },
                    };
                }
            }
            SettingsAction::ChangeMcpHttpBearerToken(index, token) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    if let McpAuthConfig::BearerToken { token: ref mut t } = http_config.auth {
                        *t = token;
                    }
                }
            }
            SettingsAction::ChangeMcpHttpOAuthScopes(index, scopes_str) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    if let McpAuthConfig::OAuth2 { ref mut scopes, .. } = http_config.auth {
                        *scopes = scopes_str
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                }
            }
            SettingsAction::ChangeMcpHttpOAuthClientName(index, name) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    if let McpAuthConfig::OAuth2 {
                        ref mut client_name,
                        ..
                    } = http_config.auth
                    {
                        *client_name = name;
                    }
                }
            }
            SettingsAction::ChangeMcpHttpOAuthRedirectPort(index, port) => {
                if let Some(McpConfig::StreamableHttp(http_config)) =
                    self.config.mcp_configs.get_mut(index)
                {
                    if let McpAuthConfig::OAuth2 {
                        ref mut redirect_port,
                        ..
                    } = http_config.auth
                    {
                        *redirect_port = port;
                    }
                }
            }
            SettingsAction::RemoveMcpConfig(index) => {
                if index < self.config.mcp_configs.len() {
                    self.config.mcp_configs.remove(index);
                }
            }
            SettingsAction::SaveSettings => {
                let llm_changed = Self::llm_configs_changed(&self.saved_config, &self.config);
                let mcp_changed = Self::mcp_configs_changed(&self.saved_config, &self.config);
                self.config.update_settings();
                // Reload the saved baseline from disk to pick up anything the
                // persistence layer may have normalized, and keep any oauth
                // tokens that were written out-of-band by the credential store.
                self.saved_config = Config::default();
                return Task::done(SettingsAction::SaveCompleted {
                    llm_changed,
                    mcp_changed,
                });
            }
            SettingsAction::SaveCompleted { .. } => {
                // No-op for settings state itself; this event is consumed by
                // the app shell to trigger model/tool reloads.
            }
            SettingsAction::StartOAuthAuth(index) => {
                let server_config = match self.saved_matching_http_config(index) {
                    Some(c) => c.clone(),
                    None => {
                        log::warn!(
                            "StartOAuthAuth({}): no saved config matches the current draft; \
                             save settings first",
                            index
                        );
                        return Task::none();
                    }
                };
                let server_name = server_config.name.clone();
                self.auth_status
                    .insert(server_name.clone(), AuthStatus::InProgress);
                return Task::perform(
                    crate::mcp::auth::run_oauth_authorization(server_config),
                    move |res| SettingsAction::OAuthAuthFinished(server_name.clone(), res),
                );
            }
            SettingsAction::OAuthAuthFinished(server_name, result) => {
                match &result {
                    Ok(_) => {
                        self.auth_status
                            .insert(server_name.clone(), AuthStatus::JustAuthenticated);
                        // Reload saved_config so the UI sees the new oauth_tokens entry
                        self.saved_config = Config::default();
                        // Fire a SaveCompleted so the app shell reloads tools.
                        return Task::done(SettingsAction::SaveCompleted {
                            llm_changed: false,
                            mcp_changed: true,
                        });
                    }
                    Err(e) => {
                        log::error!("OAuth authorization failed for '{}': {}", server_name, e);
                        self.auth_status
                            .insert(server_name, AuthStatus::Error(e.clone()));
                    }
                }
            }
            SettingsAction::ClearOAuthTokens(index) => {
                let server_config = match self.saved_matching_http_config(index) {
                    Some(c) => c.clone(),
                    None => return Task::none(),
                };
                let server_name = server_config.name.clone();
                return Task::perform(
                    crate::mcp::auth::clear_oauth_tokens(server_name.clone()),
                    move |res| SettingsAction::OAuthTokensCleared(server_name.clone(), res),
                );
            }
            SettingsAction::OAuthTokensCleared(server_name, result) => {
                match &result {
                    Ok(_) => {
                        self.auth_status.remove(&server_name);
                        // Refresh saved_config snapshot (tokens were removed on disk)
                        self.saved_config = Config::default();
                        return Task::done(SettingsAction::SaveCompleted {
                            llm_changed: false,
                            mcp_changed: true,
                        });
                    }
                    Err(e) => {
                        log::error!("Clearing OAuth tokens for '{}' failed: {}", server_name, e);
                        self.auth_status
                            .insert(server_name, AuthStatus::Error(e.clone()));
                    }
                }
            }
        }
        Task::none()
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
                    column![row![
                        text("Command:"),
                        text_input("Enter command", &stdio_config.command)
                            .on_input(move |cmd| SettingsAction::ChangeMcpStdioCommand(index, cmd)),
                        text("Args:"),
                        text_input("comma,separated,args", &args_str)
                            .on_input(move |args| SettingsAction::ChangeMcpStdioArgs(index, args)),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)]
                    .spacing(5)
                }
                McpConfig::StreamableHttp(http_config) => {
                    let auth_type: McpAuthType = (&http_config.auth).into();

                    let auth_picker = pick_list(
                        &McpAuthType::ALL[..],
                        Some(auth_type),
                        move |selected_auth| {
                            SettingsAction::ChangeMcpHttpAuthType(index, selected_auth)
                        },
                    );

                    let mut col = column![row![
                        text("Endpoint:"),
                        text_input("Enter endpoint URL", &http_config.endpoint).on_input(
                            move |endpoint| {
                                SettingsAction::ChangeMcpHttpEndpoint(index, endpoint)
                            }
                        ),
                        text("Auth:"),
                        auth_picker,
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)]
                    .spacing(5);

                    // Add auth-specific fields
                    match &http_config.auth {
                        McpAuthConfig::None => {}
                        McpAuthConfig::BearerToken { token } => {
                            col = col.push(
                                row![
                                    text("Token:"),
                                    text_input("Enter bearer token", token).on_input(move |t| {
                                        SettingsAction::ChangeMcpHttpBearerToken(index, t)
                                    }),
                                ]
                                .spacing(10)
                                .align_y(Alignment::Center),
                            );
                        }
                        McpAuthConfig::OAuth2 {
                            scopes,
                            client_name,
                            redirect_port,
                        } => {
                            let scopes_str = scopes.join(", ");
                            col = col.push(
                                row![
                                    text("Scopes:"),
                                    text_input("comma,separated,scopes", &scopes_str).on_input(
                                        move |s| {
                                            SettingsAction::ChangeMcpHttpOAuthScopes(index, s)
                                        }
                                    ),
                                    text("Client Name:"),
                                    text_input("Client name", client_name).on_input(move |n| {
                                        SettingsAction::ChangeMcpHttpOAuthClientName(index, n)
                                    }),
                                    text("Redirect Port:"),
                                    number_input(redirect_port, 1024..=65535, move |p| {
                                        SettingsAction::ChangeMcpHttpOAuthRedirectPort(index, p)
                                    }),
                                ]
                                .spacing(10)
                                .align_y(Alignment::Center),
                            );

                            // Auth action row: only enabled when this row matches a
                            // saved config. Shows status text + Authenticate / Clear buttons.
                            col = col.push(self.oauth_action_row(index, &http_config.name));
                        }
                    }

                    col
                }
            };

            column = column.push(
                column![
                    row![
                        text_input("Name", mcp_config.name()).on_input(move |name| {
                            SettingsAction::ChangeMcpConfigName(index, name)
                        }),
                        type_picker,
                        button(iced_fonts::lucide::trash())
                            .on_press(SettingsAction::RemoveMcpConfig(index))
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    config_fields,
                ]
                .spacing(5),
            );
        }

        column
            .push(button(iced_fonts::lucide::plus()).on_press(SettingsAction::AddMcpConfig))
            .spacing(10)
            .align_x(Alignment::Center)
    }

    /// Build the "Authenticate / Clear tokens / status" row for an OAuth2 MCP config.
    ///
    /// Buttons are only enabled when:
    ///   - the row has a non-empty name, AND
    ///   - the draft config matches the saved (on-disk) config exactly.
    ///
    /// This ensures `run_oauth_authorization` operates on the persisted config,
    /// not on unsaved edits.
    fn oauth_action_row(
        &self,
        index: usize,
        server_name: &str,
    ) -> iced::widget::Row<'_, SettingsAction> {
        let saved_match = self.saved_matching_http_config(index).is_some();
        let has_tokens =
            !server_name.is_empty() && self.saved_config.oauth_tokens.contains_key(server_name);
        let status = self
            .auth_status
            .get(server_name)
            .cloned()
            .unwrap_or_default();

        let (status_text, in_progress) = match &status {
            AuthStatus::Idle => {
                if !saved_match {
                    ("Save settings to enable authentication".to_string(), false)
                } else if has_tokens {
                    ("Authenticated".to_string(), false)
                } else {
                    ("Not authenticated".to_string(), false)
                }
            }
            AuthStatus::InProgress => ("Authenticating… check your browser".to_string(), true),
            AuthStatus::Error(e) => (format!("Error: {}", e), false),
            AuthStatus::JustAuthenticated => ("Authenticated".to_string(), false),
        };

        let auth_label = if has_tokens {
            "Re-authenticate"
        } else {
            "Authenticate"
        };

        let mut auth_btn = button(text(auth_label));
        if saved_match && !in_progress {
            auth_btn = auth_btn.on_press(SettingsAction::StartOAuthAuth(index));
        }

        let mut row_widgets = row![auth_btn].spacing(10).align_y(Alignment::Center);

        if has_tokens {
            let mut clear_btn = button(text("Clear tokens"));
            if !in_progress {
                clear_btn = clear_btn.on_press(SettingsAction::ClearOAuthTokens(index));
            }
            row_widgets = row_widgets.push(clear_btn);
        }

        row_widgets.push(text(status_text))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{AnthropicConfig, OpenAIConfig, VllmConfig};

    use super::*;

    #[test]
    fn test_update_theme() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeTheme(Theme::Dark));
        assert_eq!(state.config.theme, Theme::Dark);
    }

    #[test]
    fn test_update_openai_key() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeOpenAIKey("new_api_key".to_string()));
        assert_eq!(state.config.openai.api_key, "new_api_key");
    }

    #[test]
    fn test_update_openai_url() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeOpenAIUrl(
            "https://new.endpoint.com".to_string(),
        ));
        assert_eq!(state.config.openai.endpoint, "https://new.endpoint.com");
    }

    #[test]
    fn test_update_anthropic_key() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeAnthropicKey(
            "new_anthropic_key".to_string(),
        ));
        assert_eq!(state.config.anthropic.api_key, "new_anthropic_key");
    }

    #[test]
    fn test_update_anthropic_url() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeAnthropicUrl(
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
        let _ = state.update(SettingsAction::ChangeAnthropicMaxTokens(2048));
        assert_eq!(state.config.anthropic.max_tokens, 2048);
    }

    #[test]
    fn test_update_vllm_url() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeVllmUrl(
            "http://new.vllm.endpoint.com".to_string(),
        ));
        assert_eq!(state.config.vllm.endpoint, "http://new.vllm.endpoint.com");
    }

    #[test]
    fn test_update_vllm_model() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::ChangeVllmModel("new-model".to_string()));
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
                oauth_tokens: HashMap::new(),
                settings_file: "./test.json".to_string(),
            },
            saved_config: Config::default(),
            auth_status: HashMap::new(),
        };
        let _ = state.update(SettingsAction::ChangeTheme(Theme::Dark));
        let _ = state.update(SettingsAction::ChangeOpenAIKey("test_key".to_string()));
        let _ = state.update(SettingsAction::ChangeOpenAIUrl(
            "https://api.test.com".to_string(),
        ));
        let _ = state.update(SettingsAction::ChangeAnthropicKey("hello".to_string()));
        let _ = state.update(SettingsAction::ChangeAnthropicUrl(
            "https://api.anthropic.com/v1/".to_string(),
        ));
        let _ = state.update(SettingsAction::SaveSettings);
        let _ = state.update(SettingsAction::AddMcpConfig);

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

    #[test]
    fn test_change_mcp_http_auth_type() {
        let mut state = State::default();
        // Clear any pre-existing configs from settings file
        state.config.mcp_configs.clear();
        // Add a StreamableHttp config
        state
            .config
            .mcp_configs
            .push(McpConfig::StreamableHttp(McpStreamableHttpConfig::default()));

        // Change to bearer token
        let _ = state.update(SettingsAction::ChangeMcpHttpAuthType(
            0,
            McpAuthType::BearerToken,
        ));
        if let McpConfig::StreamableHttp(ref cfg) = state.config.mcp_configs[0] {
            assert!(matches!(cfg.auth, McpAuthConfig::BearerToken { .. }));
        } else {
            panic!("Expected StreamableHttp config");
        }

        // Change to OAuth2
        let _ = state.update(SettingsAction::ChangeMcpHttpAuthType(
            0,
            McpAuthType::OAuth2,
        ));
        if let McpConfig::StreamableHttp(ref cfg) = state.config.mcp_configs[0] {
            assert!(matches!(cfg.auth, McpAuthConfig::OAuth2 { .. }));
        } else {
            panic!("Expected StreamableHttp config");
        }

        // Change back to None
        let _ = state.update(SettingsAction::ChangeMcpHttpAuthType(0, McpAuthType::None));
        if let McpConfig::StreamableHttp(ref cfg) = state.config.mcp_configs[0] {
            assert!(matches!(cfg.auth, McpAuthConfig::None));
        } else {
            panic!("Expected StreamableHttp config");
        }
    }

    #[test]
    fn test_change_mcp_http_bearer_token() {
        let mut state = State::default();
        state.config.mcp_configs.clear();
        state
            .config
            .mcp_configs
            .push(McpConfig::StreamableHttp(McpStreamableHttpConfig {
                name: "test".to_string(),
                endpoint: "http://localhost:8080".to_string(),
                auth: McpAuthConfig::BearerToken {
                    token: String::new(),
                },
            }));

        let _ = state.update(SettingsAction::ChangeMcpHttpBearerToken(
            0,
            "my-secret-token".to_string(),
        ));

        if let McpConfig::StreamableHttp(ref cfg) = state.config.mcp_configs[0] {
            if let McpAuthConfig::BearerToken { ref token } = cfg.auth {
                assert_eq!(token, "my-secret-token");
            } else {
                panic!("Expected BearerToken auth");
            }
        } else {
            panic!("Expected StreamableHttp config");
        }
    }

    #[test]
    fn test_change_mcp_http_oauth_fields() {
        let mut state = State::default();
        state.config.mcp_configs.clear();
        state
            .config
            .mcp_configs
            .push(McpConfig::StreamableHttp(McpStreamableHttpConfig {
                name: "test".to_string(),
                endpoint: "http://localhost:8080".to_string(),
                auth: McpAuthConfig::OAuth2 {
                    scopes: Vec::new(),
                    client_name: "Ergon".to_string(),
                    redirect_port: 8585,
                },
            }));

        let _ = state.update(SettingsAction::ChangeMcpHttpOAuthScopes(
            0,
            "read, write, admin".to_string(),
        ));
        let _ = state.update(SettingsAction::ChangeMcpHttpOAuthClientName(
            0,
            "MyApp".to_string(),
        ));
        let _ = state.update(SettingsAction::ChangeMcpHttpOAuthRedirectPort(0, 9090));

        if let McpConfig::StreamableHttp(ref cfg) = state.config.mcp_configs[0] {
            if let McpAuthConfig::OAuth2 {
                ref scopes,
                ref client_name,
                redirect_port,
            } = cfg.auth
            {
                assert_eq!(scopes, &["read", "write", "admin"]);
                assert_eq!(client_name, "MyApp");
                assert_eq!(redirect_port, 9090);
            } else {
                panic!("Expected OAuth2 auth");
            }
        } else {
            panic!("Expected StreamableHttp config");
        }
    }

    #[test]
    fn test_llm_configs_changed_detects_diffs() {
        let a = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig {
                api_key: "a".into(),
                endpoint: "http://a".into(),
            },
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![],
            oauth_tokens: HashMap::new(),
            settings_file: "./t.json".into(),
        };
        let mut b = a.clone();
        assert!(!State::llm_configs_changed(&a, &b));
        b.openai.api_key = "changed".into();
        assert!(State::llm_configs_changed(&a, &b));

        let mut c = a.clone();
        c.anthropic.max_tokens = 42;
        assert!(State::llm_configs_changed(&a, &c));

        let mut d = a.clone();
        d.vllm.model = "x".into();
        assert!(State::llm_configs_changed(&a, &d));
    }

    #[test]
    fn test_mcp_configs_changed_detects_diffs() {
        let a = Config {
            theme: Theme::Dark,
            openai: OpenAIConfig::default(),
            anthropic: AnthropicConfig::default(),
            vllm: VllmConfig::default(),
            mcp_configs: vec![],
            oauth_tokens: HashMap::new(),
            settings_file: "./t.json".into(),
        };
        let mut b = a.clone();
        assert!(!State::mcp_configs_changed(&a, &b));
        b.mcp_configs
            .push(McpConfig::StreamableHttp(McpStreamableHttpConfig::default()));
        assert!(State::mcp_configs_changed(&a, &b));
    }

    #[test]
    fn test_start_oauth_no_saved_match_is_noop() {
        let mut state = State::default();
        state.config.mcp_configs.clear();
        state.saved_config.mcp_configs.clear();
        // Draft has an OAuth2 config that is NOT in saved_config.
        state
            .config
            .mcp_configs
            .push(McpConfig::StreamableHttp(McpStreamableHttpConfig {
                name: "unsaved".into(),
                endpoint: "http://x".into(),
                auth: McpAuthConfig::OAuth2 {
                    scopes: vec![],
                    client_name: "Ergon".into(),
                    redirect_port: 8585,
                },
            }));
        // Should not panic, nor produce any task that hits the network.
        let _ = state.update(SettingsAction::StartOAuthAuth(0));
        assert!(!state.auth_status.contains_key("unsaved"));
    }

    #[test]
    fn test_oauth_auth_finished_error_sets_status() {
        let mut state = State::default();
        let _ = state.update(SettingsAction::OAuthAuthFinished(
            "srv".to_string(),
            Err("boom".to_string()),
        ));
        match state.auth_status.get("srv") {
            Some(AuthStatus::Error(msg)) => assert_eq!(msg, "boom"),
            other => panic!("unexpected status: {:?}", other),
        }
    }
}

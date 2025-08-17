use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};

use crate::config::Config;

#[derive(Debug, Clone, Default)]
pub struct State {
    // Required to be public for dynamically changing the theme
    pub config: Config,
}

#[derive(Debug, Clone)]
pub enum Action {
    ChangeTheme(Theme),
    ChangeOpenAIKey(String),
    ChangeOpenAIUrl(String),
    ChangeAnthropicKey(String),
    ChangeAnthropicUrl(String),
    SaveSettings,
}

impl State {
    pub fn update(&mut self, action: Action) {
        match action {
            Action::ChangeTheme(theme) => {
                self.config.theme = theme;
            }
            Action::ChangeOpenAIKey(api_key) => {
                self.config.openai.api_key = api_key;
            }
            Action::ChangeOpenAIUrl(endpoint) => {
                self.config.openai.endpoint = endpoint;
            }
            Action::ChangeAnthropicKey(api_key) => {
                self.config.anthropic.api_key = api_key;
            }
            Action::ChangeAnthropicUrl(endpoint) => {
                self.config.anthropic.endpoint = endpoint;
            }
            Action::SaveSettings => {
                self.config.update_settings();
            }
        }
    }

    pub fn view(&self) -> Element<Action> {
        let col = column![
            self.theme_view(),
            self.openai_view(),
            self.anthropic_view(),
            button("Save Settings").on_press(Action::SaveSettings)
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

    fn theme_view(&self) -> iced::widget::Row<'_, Action> {
        row![
            button("Light").on_press(Action::ChangeTheme(Theme::Light)),
            button("Dark").on_press(Action::ChangeTheme(Theme::Dark)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn openai_view(&self) -> iced::widget::Row<'_, Action> {
        row![
            text("OpenAI API Key:"),
            text_input("Enter API Key", &self.config.openai.api_key)
                .on_input(|value| Action::ChangeOpenAIKey(value)),
            text("Endpoint:"),
            text_input("Enter Endpoint", &self.config.openai.endpoint)
                .on_input(|value| Action::ChangeOpenAIUrl(value)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }

    fn anthropic_view(&self) -> iced::widget::Row<'_, Action> {
        row![
            text("Anthropic API Key:"),
            text_input("Enter API Key", &self.config.anthropic.api_key)
                .on_input(|value| Action::ChangeAnthropicKey(value)),
            text("Endpoint:"),
            text_input("Enter Endpoint", &self.config.anthropic.endpoint)
                .on_input(|value| Action::ChangeAnthropicUrl(value)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{AnthropicConfig, OpenAIConfig};

    use super::*;

    #[test]
    fn test_update_theme() {
        let mut state = State::default();
        state.update(Action::ChangeTheme(Theme::Dark));
        assert_eq!(state.config.theme, Theme::Dark);
    }

    #[test]
    fn test_update_openai_key() {
        let mut state = State::default();
        state.update(Action::ChangeOpenAIKey("new_api_key".to_string()));
        assert_eq!(state.config.openai.api_key, "new_api_key");
    }

    #[test]
    fn test_update_openai_url() {
        let mut state = State::default();
        state.update(Action::ChangeOpenAIUrl(
            "https://new.endpoint.com".to_string(),
        ));
        assert_eq!(state.config.openai.endpoint, "https://new.endpoint.com");
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
                settings_file: "./test.json".to_string(),
            },
        };
        state.update(Action::ChangeTheme(Theme::Dark));
        state.update(Action::ChangeOpenAIKey("test_key".to_string()));
        state.update(Action::ChangeOpenAIUrl("https://api.test.com".to_string()));
        state.update(Action::ChangeAnthropicKey("hello".to_string()));
        state.update(Action::ChangeAnthropicUrl(
            "https://api.anthropic.com/v1/".to_string(),
        ));
        state.update(Action::SaveSettings);

        // Assuming update_settings persists the changes, we can check the config
        assert_eq!(state.config.theme, Theme::Dark);
        assert_eq!(state.config.openai.api_key, "test_key");
        assert_eq!(state.config.openai.endpoint, "https://api.test.com");

        assert_eq!(state.config.anthropic.api_key, "hello");
        assert_eq!(
            state.config.anthropic.endpoint,
            "https://api.anthropic.com/v1/"
        );
    }
}

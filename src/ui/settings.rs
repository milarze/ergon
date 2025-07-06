use iced::widget::{button, container, row};
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
}

impl State {
    pub fn update(&mut self, action: Action) {
        match action {
            Action::ChangeTheme(theme) => {
                self.config.theme = theme;
                self.config.update_settings();
            }
        }
    }

    pub fn view(&self) -> Element<Action> {
        let theme_selector = row![
            button("Light").on_press(Action::ChangeTheme(Theme::Light)),
            button("Dark").on_press(Action::ChangeTheme(Theme::Dark)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        container(theme_selector)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_theme() {
        let mut state = State::default();
        state.update(Action::ChangeTheme(Theme::Dark));
        assert_eq!(state.config.theme, Theme::Dark);
    }
}

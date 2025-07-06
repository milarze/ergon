use ergon::Ergon;

pub fn main() -> iced::Result {
    iced::application("Ergon", ergon::update, ergon::view)
        .theme(theme)
        .run()
}

fn theme(state: &Ergon) -> iced::Theme {
    state.settings.config.theme.clone()
}

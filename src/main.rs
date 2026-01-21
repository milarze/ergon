use ergon::Ergon;

pub fn main() -> iced::Result {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .expect("Failed to initialize logger");
    iced::application(ergon::init, ergon::update, ergon::view)
        .theme(theme)
        .font(iced_fonts::LUCIDE_FONT_BYTES)
        .run()
}

fn theme(state: &Ergon) -> iced::Theme {
    state.settings.config.theme.clone()
}

use ergon::Ergon;
use iced::Size;

pub fn main() -> iced::Result {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .expect("Failed to initialize logger");
    iced::application(ergon::init, ergon::update, ergon::view)
        .subscription(ergon::subscription)
        .theme(theme)
        .font(iced_fonts::LUCIDE_FONT_BYTES)
        .window(iced::window::Settings {
            size: Size::new(1600.0, 1200.0),
            min_size: Some(Size::new(400.0, 300.0)),
            resizable: true,
            ..Default::default()
        })
        .run()
}

fn theme(state: &Ergon) -> iced::Theme {
    state.settings.config.theme.clone()
}

use iced::{
    widget::{button, column, row},
    Element, Task,
};

mod chat;
mod settings;

pub use chat::{ChatMessage, Sender};

pub fn init() -> (Ergon, Task<Message>) {
    Ergon::new()
}

#[derive(Debug, Default)]
pub struct Ergon {
    current_page: PageId,
    chat: chat::State,
    pub settings: settings::State,
}

impl Ergon {
    pub fn new() -> (Self, Task<Message>) {
        let (chat_state, chat_task) = chat::State::new();
        let state = Self {
            current_page: PageId::default(),
            chat: chat_state,
            settings: settings::State::default(),
        };
        let task = chat_task.map(Message::Chat);
        (state, task)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(PageId),
    Chat(chat::Action),
    Settings(settings::Action),
}

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub enum PageId {
    #[default]
    Chat,
    Settings,
}

pub fn update(state: &mut Ergon, action: Message) -> Task<Message> {
    match action {
        Message::Navigate(page_id) => {
            state.current_page = page_id;
            Task::none()
        }
        Message::Chat(chat_action) => {
            let task = state.chat.update(chat_action);
            task.map(Message::Chat)
        }
        Message::Settings(settings_action) => {
            state.settings.update(settings_action);
            Task::none()
        }
    }
}

pub fn view(state: &Ergon) -> Element<'_, Message> {
    let navigation = build_navigation_bar(&state.current_page);

    let page_content = match &state.current_page {
        PageId::Chat => state.chat.view().map(Message::Chat),
        PageId::Settings => state.settings.view().map(Message::Settings),
    };

    column![navigation, page_content]
        .spacing(10)
        .padding(10)
        .into()
}

fn build_navigation_bar(current_page: &PageId) -> Element<'static, Message> {
    row![
        button("Chat").on_press_maybe(if current_page != &PageId::Chat {
            Some(Message::Navigate(PageId::Chat))
        } else {
            None
        }),
        button("Settings").on_press_maybe(if current_page != &PageId::Settings {
            Some(Message::Navigate(PageId::Settings))
        } else {
            None
        }),
    ]
    .spacing(10)
    .into()
}

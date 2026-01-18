use iced::{
    widget::{button, column, row},
    Element, Task,
};

mod chat;
mod settings;

use crate::{config::McpConfig, mcp::McpClient};

pub fn init() -> (Ergon, Task<NavigationAction>) {
    Ergon::new()
}

#[derive(Debug, Default)]
pub struct Ergon {
    current_page: PageId,
    chat: chat::State,
    pub settings: settings::State,
    #[allow(dead_code)]
    mcp_clients: Vec<McpClient>,
}

impl Ergon {
    pub fn new() -> (Self, Task<NavigationAction>) {
        let (chat_state, chat_task) = chat::State::new();
        let settings = settings::State::default();
        let state = Self {
            current_page: PageId::default(),
            chat: chat_state,
            settings: settings.clone(),
            mcp_clients: initialize_mcp_clients(settings.config.mcp_configs),
        };
        let task = chat_task.map(NavigationAction::Chat);
        (state, task)
    }
}

#[derive(Debug, Clone)]
pub enum NavigationAction {
    Navigate(PageId),
    Chat(chat::ChatAction),
    Settings(settings::SettingsAction),
}

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub enum PageId {
    #[default]
    Chat,
    Settings,
}

pub fn update(state: &mut Ergon, action: NavigationAction) -> Task<NavigationAction> {
    match action {
        NavigationAction::Navigate(page_id) => {
            state.current_page = page_id;
            Task::none()
        }
        NavigationAction::Chat(chat_action) => {
            let task = state.chat.update(chat_action);
            task.map(NavigationAction::Chat)
        }
        NavigationAction::Settings(settings_action) => {
            state.settings.update(settings_action);
            Task::none()
        }
    }
}

pub fn view(state: &Ergon) -> Element<'_, NavigationAction> {
    let navigation = build_navigation_bar(&state.current_page);

    let page_content = match &state.current_page {
        PageId::Chat => state
            .chat
            .view(&state.settings.config.theme)
            .map(NavigationAction::Chat),
        PageId::Settings => state.settings.view().map(NavigationAction::Settings),
    };

    column![navigation, page_content]
        .spacing(10)
        .padding(10)
        .into()
}

fn build_navigation_bar(current_page: &PageId) -> Element<'static, NavigationAction> {
    row![
        button("Chat").on_press_maybe(if current_page != &PageId::Chat {
            Some(NavigationAction::Navigate(PageId::Chat))
        } else {
            None
        }),
        button("Settings").on_press_maybe(if current_page != &PageId::Settings {
            Some(NavigationAction::Navigate(PageId::Settings))
        } else {
            None
        }),
    ]
    .spacing(10)
    .into()
}

fn initialize_mcp_clients(mcp_configs: Vec<McpConfig>) -> Vec<McpClient> {
    mcp_configs
        .iter()
        .map(|config| tokio::spawn(crate::mcp::init(config.clone())))
        .filter_map(
            |handle| match tokio::runtime::Handle::current().block_on(handle) {
                Ok(Ok(client)) => Some(client),
                Ok(Err(e)) => {
                    eprintln!("Failed to initialize MCP client: {}", e);
                    None
                }
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    None
                }
            },
        )
        .collect()
}

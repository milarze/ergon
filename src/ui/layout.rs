use iced::{
    Element, Length, Subscription, Task, widget::{Rule, button, column, container, row, rule}
};

use crate::ui::chat;
use crate::ui::settings;

pub fn init() -> (Ergon, Task<NavigationAction>) {
    Ergon::new()
}

#[derive(Debug, Default)]
pub struct Ergon {
    current_page: PageId,
    chat: chat::State,
    pub settings: settings::State,
}

impl Ergon {
    pub fn new() -> (Self, Task<NavigationAction>) {
        let (chat_state, chat_task) = chat::State::new();
        let settings = settings::State::new();
        let state = Self {
            current_page: PageId::default(),
            chat: chat_state,
            settings,
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
            // Intercept SaveCompleted before forwarding: dispatch reload tasks
            // for models/tools when the corresponding configs changed, and
            // refresh the chat-mode agent picker from the freshly-saved config.
            let reload_task = if let settings::SettingsAction::SaveCompleted {
                llm_changed,
                mcp_changed,
            } = &settings_action
            {
                let mut tasks: Vec<Task<NavigationAction>> = Vec::new();
                if *llm_changed {
                    tasks.push(
                        Task::perform(chat::load_models(), chat::ChatAction::ModelsLoaded)
                            .map(NavigationAction::Chat),
                    );
                }
                if *mcp_changed {
                    tasks.push(
                        Task::perform(chat::load_tools(), chat::ChatAction::ToolsLoaaded)
                            .map(NavigationAction::Chat),
                    );
                }
                // ACP agent list may have changed even when llm/mcp didn't.
                // Cheap to refresh unconditionally on save.
                state.chat.refresh_available_agents();
                Task::batch(tasks)
            } else {
                Task::none()
            };

            let settings_task = state
                .settings
                .update(settings_action)
                .map(NavigationAction::Settings);

            Task::batch([settings_task, reload_task])
        }
    }
}

pub fn subscription(state: &Ergon) -> Subscription<NavigationAction> {
    state.chat.subscription().map(NavigationAction::Chat)
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

    let page_content = container(page_content)
        .width(Length::FillPortion(6));
    row![navigation, rule::vertical(3), page_content]
        .into()
}

fn build_navigation_bar(current_page: &PageId) -> Element<'static, NavigationAction> {
    column![
        build_chat_history_list().map(|_| NavigationAction::Navigate(PageId::Chat)),
        button("Settings").on_press_maybe(if current_page != &PageId::Settings {
            Some(NavigationAction::Navigate(PageId::Settings))
        } else {
            None
        }).width(Length::Fill).height(Length::Fixed(40.0)),
    ]
    .width(Length::FillPortion(1))
    .spacing(3)
    .into()
}

fn build_chat_history_list() -> Element<'static, NavigationAction> {
    column![
        container("Chat History List (to be implemented)")
            .width(Length::Fill)
    ]
    .height(Length::Fill)
    .into()
}

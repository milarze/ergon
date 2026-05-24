use iced::{
    Element, Length, Subscription, Task, widget::{button, column, container, row, rule, text}
};

use crate::{chat_history::{ChatSummary, list_chat_summaries}, ui::chat};
use crate::ui::settings;

pub fn init() -> (Ergon, Task<NavigationAction>) {
    Ergon::new()
}

#[derive(Debug, Default)]
pub struct Ergon {
    current_page: PageId,
    chat: chat::State,
    pub settings: settings::State,
    chat_summaries: Vec<ChatSummary>,
}

impl Ergon {
    pub fn new() -> (Self, Task<NavigationAction>) {
        let (chat_state, chat_task) = chat::State::new();
        let settings = settings::State::new();
        let state = Self {
            current_page: PageId::default(),
            chat: chat_state,
            settings,
            chat_summaries: Vec::new(),
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
    LoadChatHistory,
    ChatHistoryLoaded(Vec<ChatSummary>),
}

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub enum ChatId {
    #[default]
    New,
    Existing(String),
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum PageId {
    Chat(ChatId),
    Settings,
}

impl Default for PageId {
    fn default() -> Self {
        PageId::Chat(ChatId::default())
    }
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
        },
        NavigationAction::LoadChatHistory => {
            Task::perform(load_chat_summaries(state.settings.config.chat_history_dir.clone()), NavigationAction::ChatHistoryLoaded)
        },
        NavigationAction::ChatHistoryLoaded(summaries) => {
            state.chat_summaries = summaries;
            Task::none()
        }
    }
}

async fn load_chat_summaries(chat_history_dir: String) -> Vec<ChatSummary> {
    list_chat_summaries(&chat_history_dir).await.unwrap_or_default()
}

pub fn subscription(state: &Ergon) -> Subscription<NavigationAction> {
    state.chat.subscription().map(NavigationAction::Chat)
}

pub fn view(state: &Ergon) -> Element<'_, NavigationAction> {
    let navigation = build_navigation_bar(&state);

    let page_content = match &state.current_page {
        PageId::Chat(_) => state
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

fn build_navigation_bar(state: &Ergon) -> Element<'static, NavigationAction> {
    column![
        build_chat_history_list(&state),
        button("Settings").on_press_maybe(if &state.current_page != &PageId::Settings {
            Some(NavigationAction::Navigate(PageId::Settings))
        } else {
            None
        }).width(Length::Fill).height(Length::Fixed(30.0)),
    ]
    .width(Length::FillPortion(1))
    .spacing(3)
    .into()
}

fn build_chat_history_list(state: &Ergon) -> Element<'static, NavigationAction> {
    let mut items = state.chat_summaries.iter().map(|summary| -> Element<'static, NavigationAction> {
        let title = if summary.summary.is_empty() {
            "Untitled Chat".to_string()
        } else {
            summary.summary.clone()
        };
        button(text(title))
            .on_press(
                NavigationAction::Navigate(
                    PageId::Chat(ChatId::Existing(summary.id.clone()))
                )
            )
            .width(Length::Fill)
            .height(Length::Fixed(30.0))
            .into()
    }).collect::<Vec<_>>();
    let new_chat_button: Element<'static, NavigationAction> = button("New Chat").on_press_maybe(if &state.current_page != &PageId::Chat(ChatId::New) {
        Some(NavigationAction::Navigate(PageId::Chat(ChatId::New)))
    } else {
        None
    }).width(Length::Fill).height(Length::Fixed(30.0)).into();
    items.insert(0, new_chat_button);
    column(items)
    .height(Length::Fill)
    .into()
}

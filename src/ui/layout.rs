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
        let chat_task = chat_task.map(NavigationAction::Chat);
        let load_summaries_task = Task::perform(load_chat_summaries(state.settings.config.chat_history_dir.clone()), NavigationAction::ChatSummariesLoaded);
        (state, Task::batch([chat_task, load_summaries_task]))
    }
}

#[derive(Debug, Clone)]
pub enum NavigationAction {
    Navigate(PageId),
    Chat(chat::ChatAction),
    Settings(settings::SettingsAction),
    LoadChatSummaries,
    ChatSummariesLoaded(Vec<ChatSummary>),
    ChatHistoryLoaded(Option<crate::chat_history::ChatHistory>),
    DeleteChatHistory(String),
    ChatHistoryDeleted(String),
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
            state.current_page = page_id.clone();
            match page_id {
                PageId::Chat(chat_id) => {
                    match chat_id {
                        ChatId::New => Task::perform(async { None }, NavigationAction::ChatHistoryLoaded),
                        ChatId::Existing(id) => Task::perform(load_chat_history(id.clone(), state.settings.config.chat_history_dir.clone()), NavigationAction::ChatHistoryLoaded),
                    }
                }
                PageId::Settings => Task::none(),
            }
        }
        NavigationAction::Chat(chat_action) => {
            let reload_summaries_task = if let chat::ChatAction::ChatHistorySaved(Ok(chat_history)) = &chat_action {
                state.current_page = PageId::Chat(ChatId::Existing(chat_history.id.clone()));
                Task::perform(load_chat_summaries(state.settings.config.chat_history_dir.clone()), NavigationAction::ChatSummariesLoaded)
            } else {
                Task::none()
            };

            let chat_task = state.chat.update(chat_action).map(NavigationAction::Chat);

            Task::batch([chat_task, reload_summaries_task])
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
        NavigationAction::LoadChatSummaries => {
            Task::perform(load_chat_summaries(state.settings.config.chat_history_dir.clone()), NavigationAction::ChatSummariesLoaded)
        },
        NavigationAction::ChatSummariesLoaded(summaries) => {
            state.chat_summaries = summaries;
            Task::none()
        },
        NavigationAction::ChatHistoryLoaded(chat_history) => {
            let task = state.chat.update(chat::ChatAction::LoadChatHistory(chat_history));
            task.map(NavigationAction::Chat)
        },
        NavigationAction::DeleteChatHistory(chat_history_id) => {
            let delete_task = Task::perform(
                delete_chat_history(chat_history_id.clone(), state.settings.config.chat_history_dir.clone()),
                NavigationAction::ChatHistoryDeleted,
            );
            Task::batch([delete_task])
        },
        NavigationAction::ChatHistoryDeleted(chat_history_id) => {
            let _ = state.chat.update(chat::ChatAction::ChatHistoryDeleted(chat_history_id.clone()));
            if let PageId::Chat(ChatId::Existing(current_id)) = &state.current_page {
                if *current_id == chat_history_id {
                    state.current_page = PageId::Chat(ChatId::New);
                }
            }
            Task::perform(
                load_chat_summaries(state.settings.config.chat_history_dir.clone()),
                NavigationAction::ChatSummariesLoaded,
            )
        },
    }
}

async fn load_chat_summaries(chat_history_dir: String) -> Vec<ChatSummary> {
    list_chat_summaries(&chat_history_dir).await.unwrap_or_default()
}

async fn load_chat_history(chat_id: String, chat_history_dir: String) -> Option<crate::chat_history::ChatHistory> {
    crate::chat_history::get_chat_history(&chat_id, &chat_history_dir).await.unwrap_or(None)
}

async fn delete_chat_history(chat_id: String, chat_history_dir: String) -> String {
    match crate::chat_history::delete_chat_history(&chat_id, &chat_history_dir).await {
        Ok(_) => chat_id,
        Err(_) => String::new(), // Return empty string on failure; could also choose to return Result<String, Error> and handle in update()
    }
}

pub fn subscription(state: &Ergon) -> Subscription<NavigationAction> {
    state.chat.subscription().map(NavigationAction::Chat)
}

pub fn view(state: &Ergon) -> Element<'_, NavigationAction> {
    let navigation = build_navigation_bar(state);

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
        build_chat_history_list(state),
        button("Settings").on_press_maybe(if state.current_page != PageId::Settings {
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
        row![
            button(text(title))
                .on_press_maybe(
                    if state.current_page != PageId::Chat(ChatId::Existing(summary.id.clone())) {
                        Some(NavigationAction::Navigate(PageId::Chat(ChatId::Existing(summary.id.clone()))))
                    } else {
                        None
                    }
                )
                .width(Length::Fill)
                .height(Length::Fixed(30.0)),
            button("-")
                .on_press(NavigationAction::DeleteChatHistory(summary.id.clone()))
                .width(Length::Fixed(30.0))
                .height(Length::Fixed(30.0)),
        ].into()
    }).collect::<Vec<_>>();
    let new_chat_button: Element<'static, NavigationAction> = button("New Chat").on_press_maybe(if state.current_page != PageId::Chat(ChatId::New) {
        Some(NavigationAction::Navigate(PageId::Chat(ChatId::New)))
    } else {
        None
    }).width(Length::Fill).height(Length::Fixed(30.0)).into();
    items.insert(0, new_chat_button);
    column(items)
    .height(Length::Fill)
    .into()
}

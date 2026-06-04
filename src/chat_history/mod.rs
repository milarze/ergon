use crate::models::Message;
use anyhow::Result;
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatHistory {
    pub id: String,
    pub messages: Vec<Message>,
    pub summary: Option<String>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Default for ChatHistory {
    fn default() -> Self {
        ChatHistory {
            id: Alphanumeric.sample_string(&mut rand::rng(), 16),
            messages: Vec::new(),
            summary: None,
            last_updated: chrono::Utc::now(),
        }
    }
}

impl ChatHistory {
    pub fn from_messages(messages: Vec<Message>, chat_id: Option<String>) -> Self {
        ChatHistory {
            summary: if let Some(message) =  messages.first() {
                Some(message.content.iter().filter_map(|c| c.as_text()).collect::<Vec<_>>().join(" "))
            } else {
                Some("No messages yet".to_string())
            },
            messages,
            id: chat_id.unwrap_or_else(|| Alphanumeric.sample_string(&mut rand::rng(), 16)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatSummary {
    pub id: String,
    pub summary: String,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

pub async fn store_chat_history(chat_history: ChatHistory, chat_history_dir: &str) -> Result<()> {
    if !std::fs::exists(chat_history_dir).unwrap_or(false) {
        return Err(anyhow::anyhow!("Chat history directory does not exist: {}", chat_history_dir));
    }

    Ok(std::fs::write(
        chat_history_file_path(&chat_history.id, chat_history_dir),
        serde_json::to_string(&chat_history)?,
    )?)
}

pub async fn list_chat_summaries(chat_history_dir: &str) -> Result<Vec<ChatSummary>> {
    let mut summaries = Vec::new();
    if std::fs::exists(chat_history_dir).unwrap_or(false) {
        for entry in std::fs::read_dir(chat_history_dir)? {
            let entry = entry?;
            if entry.path().is_file() && entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(entry.path())?;
                let chat_history: ChatHistory = serde_json::from_str(&content)?;
                summaries.push(ChatSummary {
                    id: chat_history.id,
                    summary: chat_history.summary.unwrap_or_else(|| "No summary available".to_string()),
                    last_updated: chat_history.last_updated,
                });
            }
        }
    }
    Ok(summaries)
}

pub async fn delete_chat_history(chat_history_id: &str, chat_history_dir: &str) -> Result<()> {
    let file_path = chat_history_file_path(chat_history_id, chat_history_dir);
    if std::fs::exists(&file_path).unwrap_or(false) {
        std::fs::remove_file(file_path)?;
        Ok(())
    } else {
        Err(anyhow::anyhow!("Chat history not found: {}", chat_history_id))
    }
}

pub async fn get_chat_history(chat_history_id: &str, chat_history_dir: &str) -> Result<Option<ChatHistory>> {
    let file_path = chat_history_file_path(chat_history_id, chat_history_dir);
    if std::fs::exists(&file_path).unwrap_or(false) {
        let content = std::fs::read_to_string(file_path)?;
        let chat_history: ChatHistory = serde_json::from_str(&content)?;
        Ok(Some(chat_history))
    } else {
        Ok(None)
    }
}

pub fn chat_history_file_path(chat_history_id: &str, chat_history_dir: &str) -> String {
    format!("{}/{}.json", chat_history_dir, chat_history_id)
}

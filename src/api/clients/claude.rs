//! The Claude API client.

use crate::{config::Config, ui::ChatMessage};

use super::{ErgonClient, Models};

#[derive(Debug, Clone)]
pub struct ClaudeClient {
    config: Config,
}

impl ErgonClient for ClaudeClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        _model: Models,
    ) -> Result<String, String> {
        // Here you would implement the actual API call to Claude
        // For now, we return a dummy response
        if messages.is_empty() {
            Err("No messages provided".to_string())
        } else {
            Ok(format!("Response to: {:?}", messages))
        }
    }
}

impl Default for ClaudeClient {
    fn default() -> Self {
        ClaudeClient {
            config: Config::default(),
        }
    }
}

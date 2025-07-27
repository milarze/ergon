use strum_macros::{Display, EnumIter, EnumString};

use crate::ui::ChatMessage;

pub mod anthropic;
pub mod openai;

pub trait ErgonClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: Models,
    ) -> Result<String, String>;
}

#[derive(Debug, EnumIter, Clone)]
pub enum Clients {
    OpenAI,
    Anthropic,
}

impl Clients {
    pub async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: Models,
    ) -> Result<String, String> {
        match self {
            Clients::OpenAI => {
                openai::OpenAIClient::default()
                    .complete_message(messages, model)
                    .await
            }
            Clients::Anthropic => {
                anthropic::AnthropicClient::default()
                    .complete_message(messages, model)
                    .await
            }
        }
    }
}

#[derive(Debug, EnumIter, Clone, Eq, PartialEq, EnumString, Display)]
pub enum Models {
    #[strum(serialize = "o4-mini")]
    O4Mini,
    #[strum(serialize = "opus-4")]
    Opus4,
}

impl Models {
    pub fn client(&self) -> Clients {
        match self {
            Models::O4Mini => Clients::OpenAI,
            Models::Opus4 => Clients::Anthropic,
        }
    }
}

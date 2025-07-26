use strum_macros::{Display, EnumIter, EnumString};

use crate::ui::ChatMessage;

pub mod claude;
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
        }
    }
}

#[derive(Debug, EnumIter, Clone, Eq, PartialEq, EnumString, Display)]
pub enum Models {
    #[strum(serialize = "o4-mini")]
    O4Mini,
}

impl Models {
    pub fn client(&self) -> Clients {
        match self {
            Models::O4Mini => Clients::OpenAI,
        }
    }
}

use std::sync::{Arc, RwLock};
use strum_macros::EnumIter;

use crate::ui::ChatMessage;

pub mod anthropic;
pub mod openai;

pub trait ErgonClient {
    async fn complete_message(
        &self,
        messages: Vec<ChatMessage>,
        model: &str,
    ) -> Result<String, String>;

    async fn list_models(&self) -> Result<Vec<Model>, String>;
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
        model: &str,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModel {
    pub model: Model,
    pub client: Clients,
}

#[derive(Debug)]
pub struct ModelManager {
    models: Arc<RwLock<Vec<AvailableModel>>>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn fetch_models(&self) -> Result<(), String> {
        let mut all_models = Vec::new();

        let openai_client = openai::OpenAIClient::default();
        match openai_client.list_models().await {
            Ok(models) => {
                for model in models {
                    all_models.push(AvailableModel {
                        model,
                        client: Clients::OpenAI,
                    });
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch OpenAI models: {}", e);
            }
        }

        let anthropic_client = anthropic::AnthropicClient::default();
        match anthropic_client.list_models().await {
            Ok(models) => {
                for model in models {
                    all_models.push(AvailableModel {
                        model,
                        client: Clients::Anthropic,
                    });
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch Anthropic models: {}", e);
            }
        }

        let mut models = self
            .models
            .write()
            .map_err(|_| "Failed to acquire write lock")?;
        *models = all_models;

        Ok(())
    }

    pub fn get_models(&self) -> Result<Vec<AvailableModel>, String> {
        let models = self
            .models
            .read()
            .map_err(|_| "Failed to acquire read lock")?;
        Ok(models.clone())
    }

    pub fn find_model(&self, name: &str) -> Result<Option<AvailableModel>, String> {
        let models = self
            .models
            .read()
            .map_err(|_| "Failed to acquire read lock")?;
        Ok(models.iter().find(|m| m.model.name == name).cloned())
    }
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

static MODEL_MANAGER: std::sync::OnceLock<ModelManager> = std::sync::OnceLock::new();

pub fn get_model_manager() -> &'static ModelManager {
    MODEL_MANAGER.get_or_init(ModelManager::new)
}

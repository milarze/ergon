use std::sync::{Arc, RwLock};
mod openai_compatible;

use crate::config::Config;
pub use crate::models::{Clients, CompletionRequest, CompletionResponse, ModelInfo};

pub mod anthropic;
pub mod openai;
pub mod vllm;

pub trait ErgonClient {
    async fn complete_message(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse>;

    async fn list_models(&self) -> anyhow::Result<Vec<Model>>;
}

impl Clients {
    pub async fn complete_message(
        &self,
        request: CompletionRequest,
    ) -> anyhow::Result<CompletionResponse> {
        match self {
            Clients::OpenAI(index) => {
                if let Some(openai_config) = Config::default().openai_compatible.get(*index) {
                    let openai_client = openai::OpenAIClient::new(openai_config.to_owned());
                    openai_client
                        .complete_message(request)
                        .await
                } else {
                    Err(anyhow::anyhow!("Invalid OpenAI client index: {}", index))
                }
            }
            Clients::Anthropic => {
                anthropic::AnthropicClient::default()
                    .complete_message(request)
                    .await
            }
            Clients::Vllm => vllm::VllmClient::default().complete_message(request).await,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub id: String,
}

#[derive(Debug)]
pub struct ModelManager {
    models: Arc<RwLock<Vec<ModelInfo>>>,
}

impl ModelManager {
    fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn fetch_models(&self) -> Result<(), String> {
        let mut all_models = Vec::new();

        for (index, openai_config) in Config::default().openai_compatible.iter().enumerate() {
            let openai_client = openai::OpenAIClient::new(openai_config.clone());
            let mut models = self.get_openai_compatible_models(index, openai_client).await;
            all_models.append(&mut models);
        }

        let anthropic_client = anthropic::AnthropicClient::default();
        match anthropic_client.list_models().await {
            Ok(models) => {
                for model in models {
                    all_models.push(ModelInfo {
                        name: model.name,
                        id: model.id,
                        client: crate::models::Clients::Anthropic,
                    });
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch Anthropic models: {}", e);
            }
        }

        let vllm_client = vllm::VllmClient::default();
        match vllm_client.list_models().await {
            Ok(models) => {
                for model in models {
                    all_models.push(ModelInfo {
                        name: model.name,
                        id: model.id,
                        client: crate::models::Clients::Vllm, // Assuming vLLM uses OpenAI client type
                    });
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch vLLM models: {}", e);
            }
        }

        let mut models = self
            .models
            .write()
            .map_err(|_| "Failed to acquire write lock")?;
        *models = all_models;

        Ok(())
    }

    pub fn get_models(&self) -> Result<Vec<ModelInfo>, String> {
        let models = self
            .models
            .read()
            .map_err(|_| "Failed to acquire read lock")?;
        Ok(models.clone())
    }

    pub fn find_model(&self, name: &str) -> Result<Option<ModelInfo>, String> {
        let models = self
            .models
            .read()
            .map_err(|_| "Failed to acquire read lock")?;
        Ok(models.iter().find(|m| m.name == name).cloned())
    }

    async fn get_openai_compatible_models(&self, index: usize, openai_client: openai::OpenAIClient) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        if let Ok(openai_models) = openai_client.list_models().await {
            for model in openai_models {
                models.push(ModelInfo {
                    name: model.name,
                    id: model.id,
                    client: crate::models::Clients::OpenAI(index),
                });
            }
        }
        models
    }
}

static MODEL_MANAGER: std::sync::OnceLock<ModelManager> = std::sync::OnceLock::new();

pub fn get_model_manager() -> &'static ModelManager {
    MODEL_MANAGER.get_or_init(ModelManager::new)
}

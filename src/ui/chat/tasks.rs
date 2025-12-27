use crate::{
    api::clients::get_model_manager,
    models::{Clients, CompletionRequest, CompletionResponse, ModelInfo, Tool},
    ui::chat::ChatMessage,
};

pub async fn complete_message(
    messages: Vec<ChatMessage>,
    client: Clients,
    model: String,
    tools: Vec<Tool>,
) -> CompletionResponse {
    let request = CompletionRequest {
        messages: messages.iter().map(|m| m.clone().into()).collect(),
        model,
        temperature: None,
        tools: Some(tools),
    };
    let result = client.complete_message(request).await;
    match result {
        Ok(response) => response,
        Err(err) => CompletionResponse {
            id: "error".to_string(),
            object: err.to_string(),
            created: 0,
            model: "".to_string(),
            choices: vec![],
        },
    }
}

pub async fn load_models() -> Vec<ModelInfo> {
    let manager = get_model_manager();
    match manager.fetch_models().await {
        Ok(_) => {
            match manager.get_models() {
                Ok(models) => models,
                Err(_) => {
                    // Fallback to hardcoded models
                    vec![
                        ModelInfo {
                            name: "gpt-4o-mini".to_string(),
                            id: "gpt-4o-mini".to_string(),
                            client: Clients::OpenAI,
                        },
                        ModelInfo {
                            name: "Claude 3.5 Sonnet".to_string(),
                            id: "claude-3-5-sonnet-20241022".to_string(),
                            client: Clients::Anthropic,
                        },
                    ]
                }
            }
        }
        Err(_) => {
            // Fallback to hardcoded models
            vec![
                ModelInfo {
                    name: "gpt-4o-mini".to_string(),
                    id: "gpt-4o-mini".to_string(),
                    client: Clients::OpenAI,
                },
                ModelInfo {
                    name: "Claude 3.5 Sonnet".to_string(),
                    id: "claude-3-5-sonnet-20241022".to_string(),
                    client: Clients::Anthropic,
                },
            ]
        }
    }
}

pub async fn load_tools() -> Vec<crate::models::Tool> {
    let manager = crate::mcp::get_tool_manager();
    match manager.load_tools().await {
        Ok(_) => match manager.get_tools() {
            Ok(tools) => tools,
            Err(_) => vec![],
        },
        Err(_) => vec![],
    }
}

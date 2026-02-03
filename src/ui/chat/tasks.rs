use rmcp::model::JsonObject;
use serde_json::Value;

use crate::{
    api::clients::get_model_manager,
    models::{
        Clients, CompletionRequest, CompletionResponse, Content, ModelInfo, Tool, ToolCall,
        ToolCallResult,
    },
    ui::chat::models::ChatMessage,
};

pub async fn complete_message(
    messages: Vec<ChatMessage>,
    client: Clients,
    model: String,
    tools: Vec<Tool>,
) -> CompletionResponse {
    log::info!(
        "message roles: {:?}",
        messages
            .iter()
            .map(|m| m.message.role.clone())
            .collect::<Vec<String>>()
    );
    log::info!(
        "message contents: {:?}",
        messages
            .iter()
            .map(|m| m.message.content.clone())
            .collect::<Vec<Vec<Content>>>()
    );
    let request = CompletionRequest {
        messages: messages.iter().map(|cm| cm.clone().into()).collect(),
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
        Ok(_) => manager.get_tools().unwrap_or_default(),
        Err(_) => vec![],
    }
}

pub async fn call_tool(tool_call: ToolCall) -> Result<ToolCallResult, (String, String)> {
    log::info!("Received tool call: {:?}", tool_call);
    let manager = crate::mcp::get_tool_manager();
    let call_id = tool_call.id.clone();
    let client = manager
        .get_client_by_tool_call(&tool_call.function.name)
        .map_err(|e| (call_id.clone(), e.to_string()))?
        .ok_or_else(|| {
            (
                call_id.clone(),
                "Client not found for tool call".to_string(),
            )
        })?;
    let args_json: JsonObject<Value> = serde_json::from_str(&tool_call.function.arguments)
        .map_err(|e| (call_id.clone(), format!("Failed to parse arguments: {}", e)))?;
    log::info!("Tool call arguments as JSON: {:?}", args_json);
    let function_name = tool_call.function.name.clone();
    let (_, client_function_name) = manager
        .tool_client_and_name_by_tool_call(function_name)
        .map_err(|e| {
            (
                call_id.clone(),
                format!("Failed to extract client function name: {}", e),
            )
        })?
        .ok_or_else(|| {
            (
                call_id.clone(),
                "Function name mapping not found for tool call".to_string(),
            )
        })?;
    let request_params = rmcp::model::CallToolRequestParams {
        name: client_function_name.clone().into(),
        arguments: Some(args_json.clone()),
        meta: None,
        task: None,
    };
    log::info!(
        "Calling tool: {} with args: {:?}",
        client_function_name,
        request_params.arguments
    );
    let tool_result = client
        .call_tool(request_params)
        .await
        .map_err(|e| (call_id.clone(), e.to_string()))?;
    let json_string = serde_json::to_string(&tool_result).map_err(|e| {
        (
            call_id.clone(),
            format!("Failed to serialize tool result: {}", e),
        )
    })?;
    Ok(ToolCallResult {
        success: true,
        id: call_id.clone(),
        contents: vec![Content::tool_result(call_id, json_string)],
    })
}

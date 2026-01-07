use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    Json,
};
use base64::prelude::*;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::time::SystemTime;
use tokio_stream::StreamExt;
use tonic::Request;

use indigo_common::{
    inference::inference_service_client::InferenceServiceClient,
    inference::InferenceRequest as GrpcInferenceRequest,
    inference::InferenceResponse as GrpcInferenceResponse, ChatMessage,
};

use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{AppState, convert_common_tool_to_protobuf};

#[derive(Deserialize, Debug)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    pub stream: Option<bool>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub tools: Option<Vec<serde_json::Value>>,
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolCall {
    pub index: u32,
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Debug)]
pub struct OpenAIStreamResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Serialize, Debug)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Delta {
    pub content: Option<String>,
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize, Debug)]
pub struct OpenAIResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Serialize, Debug)]
pub struct Choice {
    pub index: u32,
    pub message: OpenAIMessage,
    pub finish_reason: String,
}

#[derive(Serialize, Debug)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Serialize, Debug)]
pub struct OpenAIModel {
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

#[derive(Serialize, Debug)]
pub struct OpenAIModelList {
    pub data: Vec<OpenAIModel>,
}

#[derive(Serialize, Debug)]
pub struct OpenAIError {
    pub message: String,
    pub r#type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIError,
}

impl OpenAIErrorResponse {
    pub fn new(message: impl Into<String>, code: Option<String>) -> Self {
        Self {
            error: OpenAIError {
                message: message.into(),
                r#type: "server_error".to_string(),
                param: None,
                code,
            },
        }
    }
}

pub async fn list_openai_models(State(state): State<AppState>) -> Json<OpenAIModelList> {
    let mut models = Vec::new();

    let scan_dirs = vec![
        ".".to_string(),
        "./models".to_string(),
        "./indigo-core/target/debug/models".to_string(),
        "./indigo-core/crates/indigo-hub/models".to_string(),
        "./indigo-core/crates/indigo-node/models".to_string(),
    ];

    fn visit_dirs(dir: &std::path::Path, models: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, models);
                } else if let Some(ext) = path.extension() {
                    if ext == "gguf" {
                        // let s = path.to_string_lossy().replace("\\", "/");
                        // let clean = s.trim_start_matches("./");
                         let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        models.push(crate::prettify_model_name(&filename));
                    }
                }
            }
        }
    }
    for dir in scan_dirs {
        visit_dirs(std::path::Path::new(&dir), &mut models);
    }

    // Add online nodes' models (these are verified to be running)
    let nodes = state.nodes.read().unwrap();
    for node in nodes.values() {
        if node.status == "Online" {
            let filename = std::path::Path::new(&node.model_name)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let pretty_name = crate::prettify_model_name(&filename);
            // Only add node models if they're not already in the list
            if !models.contains(&pretty_name) {
                models.push(pretty_name);
            }
        }
    }

    let agents = state.agents.read().unwrap();
    for agent in agents.values() {
        models.push(agent.id.clone());
    }

    models.sort();
    models.dedup();

    let data = models
        .into_iter()
        .map(|id| OpenAIModel {
            id,
            object: "model".to_string(),
            owned_by: "local".to_string(),
        })
        .collect();

    Json(OpenAIModelList { data })
}

pub async fn list_proxy_models(State(state): State<ProxyState>) -> Json<OpenAIModelList> {
    let data = vec![OpenAIModel {
        id: state.model_name.clone(),
        object: "model".to_string(),
        owned_by: "local".to_string(),
    }];
    Json(OpenAIModelList { data })
}

#[derive(Clone)]
pub struct ProxyState {
    pub target_address: String,
    pub model_name: String,
    pub tool_registry: crate::tool_registry::SharedToolRegistry,
}

pub struct ToolParser {
    buffer: String,
}

impl ToolParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn push(&mut self, token: &str) -> (Option<String>, Option<ToolCall>) {
        if token.len() > 50 {
            println!("DEBUG: Processing token: {}", &token[..50.min(token.len())]);
        }
        self.buffer.push_str(token);

        // 1. First check for MCP JSON format: {"function_name": "...", "arguments_json": "..."}
        let trimmed = self.buffer.trim();
        if trimmed.starts_with("{") && trimmed.ends_with("}") {
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(fname) = json_value.get("function_name").and_then(|v| v.as_str()) {
                    let args = if let Some(args_obj) = json_value.get("arguments") {
                        if args_obj.is_string() {
                            args_obj.as_str().unwrap_or("{}").to_string()
                        } else {
                            args_obj.to_string()
                        }
                    } else {
                        json_value
                            .get("arguments_json")
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}")
                            .to_string()
                    };
                    
                    let tool_call = ToolCall {
                        index: 0,
                        id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                        r#type: "function".to_string(),
                        function: FunctionCall { 
                            name: fname.to_string(), 
                            arguments: args,
                        },
                    };
                    
                    self.buffer.clear();
                    return (None, Some(tool_call));
                }
            }
        }

        (None, None)
    }
    
    pub fn flush(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            let s = self.buffer.clone();
            self.buffer.clear();
            Some(s)
        }
    }

    pub fn parse_static(content: &str) -> (String, Option<Vec<ToolCall>>, String) {
        use regex::Regex;
        // 1. Fuzzy JSON Extraction using Regex
        // Finds the first block that looks like a JSON object: { ... }
        // (?s) enables dot to match newlines
        let re = Regex::new(r"(?s)\{.*\}").unwrap();
        
        if let Some(mat) = re.find(content) {
            let json_str = mat.as_str();
            
            // Try to parse the extracted block as JSON
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
                // Check if it looks like a tool call (has function_name)
                if let Some(fname) = json_value.get("function_name").and_then(|v| v.as_str()) {
                     let args = if let Some(args_obj) = json_value.get("arguments") {
                        if args_obj.is_string() {
                            args_obj.as_str().unwrap_or("{}").to_string()
                        } else {
                            args_obj.to_string()
                        }
                    } else {
                        json_value
                            .get("arguments_json")
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}")
                            .to_string()
                    };
                    
                    let tool_call = ToolCall {
                        index: 0,
                        id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                        r#type: "function".to_string(),
                        function: FunctionCall { 
                            name: fname.to_string(), 
                            arguments: args 
                        },
                    };

                    // Capture text BEFORE the JSON block as content
                    let prefix = &content[..mat.start()];
                    let final_content = prefix.trim().to_string();
                    
                    return (final_content, Some(vec![tool_call]), "tool_calls".to_string());
                }
            }
        }
        
        (content.to_string(), None, "stop".to_string())
    }
}

pub async fn proxy_chat_completions(
    State(state): State<ProxyState>,
    Json(req): Json<OpenAIChatRequest>,
) -> impl IntoResponse {
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 1. Prepare Content
    let mut image_data: Option<Vec<u8>> = None;
    let mut indigo_messages: Vec<ChatMessage> = Vec::new();

    // Inject System Instruction for Tools
    if req.tools.is_some() {
        indigo_messages.push(ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String("You have access to tools. To use a tool, output a JSON object with 'function_name' and 'arguments'.".to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for m in req.messages {
        let mut text_content = String::new();
        if let Some(s) = m.content.as_str() {
            text_content = s.to_string();
        } else if let Some(arr) = m.content.as_array() {
            for part in arr {
                if let Some(t) = part.get("type").and_then(|v| v.as_str()) {
                    if t == "text" {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            text_content.push_str(text);
                        }
                    } else if t == "image_url" {
                        if let Some(url) = part
                            .get("image_url")
                            .and_then(|v| v.get("url"))
                            .and_then(|v| v.as_str())
                        {
                            if url.starts_with("data:image") {
                                let parts: Vec<&str> = url.split(",").collect();
                                if parts.len() > 1 {
                                    if let Ok(bytes) = BASE64_STANDARD.decode(parts[1]) {
                                        image_data = Some(bytes);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        let tool_calls = if let Some(tc) = m.tool_calls {
            Some(serde_json::to_value(tc).unwrap_or(serde_json::Value::Null))
        } else {
            None
        };

        indigo_messages.push(ChatMessage {
            role: m.role,
            content: serde_json::Value::String(text_content),
            tool_calls,
            tool_call_id: m.tool_call_id,
        });
    }

let prompt_payload = serde_json::to_string(&indigo_messages).unwrap_or_default();
    let stream_req = req.stream.unwrap_or(false);
    let model_name = state.model_name.clone();

    // 2. Connect to Node (Directly)
    let mut client = match InferenceServiceClient::connect(state.target_address.clone()).await {
        Ok(c) => c,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(OpenAIErrorResponse::new(format!("Failed to connect to proxy target: {}", e), Some("connection_error".into()))),
            ).into_response();
        }
    };

    // Get tools to include in request
    let tools = {
        let registry = state.tool_registry.read().unwrap();
        registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
    };

    let grpc_req = GrpcInferenceRequest {
        prompt: prompt_payload,
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature.unwrap_or(0.7),
        image_data: image_data.unwrap_or_default(),
        stop: vec![],
        model_name: model_name.clone(),
        tools: tools,
    };

    if stream_req {
        let stream: Pin<Box<dyn Stream<Item = Result<Event, axum::Error>> + Send>> =
            Box::pin(async_stream::try_stream! {
                match client.run_inference(Request::new(grpc_req)).await {
                    Ok(resp) => {
                        let mut grpc_stream = resp.into_inner();
                        let mut parser = ToolParser::new();
                        let mut tool_emitted = false;
                        let mut tool_index = 0;

                        while let Some(Ok(item)) = grpc_stream.next().await {
                            if item.status == 1 {
                                // Flush parser
                                if let Some(s) = parser.flush() {
                                     yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: Some(s), role: None, tool_calls: None },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }

                                 yield Event::default().json_data(OpenAIStreamResponse {
                                    id: id.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model_name.clone(),
                                    choices: vec![StreamChoice {
                                        index: 0,
                                        delta: Delta { content: None, role: None, tool_calls: None },
                                        finish_reason: Some(if tool_emitted { "tool_calls".to_string() } else { "stop".to_string() }),
                                    }],
                                }).map_err(axum::Error::new)?;
                                break;
                            }

                            if !item.token.is_empty() {
                                let (text, tool) = parser.push(&item.token);
                                
                                if let Some(t) = text {
                                    yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: Some(t), role: None, tool_calls: None },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }
                                
                                if let Some(mut tc) = tool {
                                    tc.index = tool_index;
                                    tool_index += 1;
                                    tool_emitted = true;
                                    yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: None, role: None, tool_calls: Some(vec![tc]) },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }
                            }
                        }
                    }
                    Err(e) => {
                         eprintln!("Proxy gRPC Stream Error: {}", e);
                    }
                }
                yield Event::default().data("[DONE]");
            });
        Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::default())
            .into_response()
    } else {
        match client.run_inference(Request::new(grpc_req)).await {
            Ok(resp) => {
                let mut grpc_stream = resp.into_inner();
                let mut full_content = String::new();
                while let Some(Ok(item)) = grpc_stream.next().await {
                    if item.status == 1 {
                        break;
                    }
                    full_content.push_str(&item.token);
                }

                let (final_content, tool_calls, finish_reason) = ToolParser::parse_static(&full_content);

                let response = OpenAIResponse {
                    id,
                    object: "chat.completion".to_string(),
                    created,
                    model: model_name,
                    choices: vec![Choice {
                        index: 0,
                        message: OpenAIMessage {
                            role: "assistant".to_string(),
                            content: if final_content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(final_content) },
                            tool_calls,
                            tool_call_id: None,
                        },
                        finish_reason,
                    }],
                    usage: Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                };
                Json(response).into_response()
            }
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenAIErrorResponse::new(format!("Inference failed: {}", e), Some("inference_error".into()))),
            ).into_response(),
        }
    }
}

pub async fn chat_completions(
    State(state): State<AppState>,
    Json(req): Json<OpenAIChatRequest>,
) -> impl IntoResponse {
    // 0. Parse Messages and Check for Image
    let mut image_data: Option<Vec<u8>> = None;
    let mut indigo_messages: Vec<ChatMessage> = Vec::new();

    // Agent / Tool Logic
    let mut resolved_model = req.model.clone();
    let mut system_prompt_override = None;
    
    // Check if model matches an Agent ID
    {
        let agents = state.agents.read().unwrap();
        if let Some(agent) = agents.get(&req.model) {
            println!("OpenAI Request routed to Agent: {}", agent.name);
            resolved_model = agent.model.clone();
            system_prompt_override = Some(agent.system_prompt.clone());
        }
    }

    // Inject System Instruction for Tools
    let mut should_inject_tools = true;
    let mut force_tool = false;

    match &req.tool_choice {
        Some(serde_json::Value::String(s)) => {
            if s == "none" {
                should_inject_tools = false;
            } else if s == "required" {
                force_tool = true;
            }
        }
        Some(serde_json::Value::Object(obj)) => {
             // Specific tool choice, e.g. {"type": "function", "function": {"name": "..."}}
             if let Some(t) = obj.get("type").and_then(|v| v.as_str()) {
                 if t == "function" {
                     force_tool = true;
                 }
             }
        }
        None => {
            // Default is "auto", which means tools are available but not forced
        }
        _ => {}
    }

    if let Some(tools) = &req.tools {
        if should_inject_tools {
            let tools_json = serde_json::to_string(tools).unwrap_or_default();
            let mut tool_instruction = format!(
                "\n\nYou have access to the following tools:\n{}\n\nTo use a tool, you must output a JSON object with the \"function_name\" and \"arguments\" keys.\nExample: {{ \"function_name\": \"read_file\", \"arguments\": {{ \"path\": \"README.md\" }} }}",
                tools_json
            );

            if force_tool {
                tool_instruction.push_str("\nYou MUST call a tool in your response.");
            }
            
            if let Some(sp) = &mut system_prompt_override {
                sp.push_str(&tool_instruction);
            } else {
                 system_prompt_override = Some(format!("You are a helpful assistant.{}", tool_instruction));
            }
        }
    }

    /* Inject Tools if present
    if let Some(tools) = &req.tools {
        let should_inject = match &req.tool_choice {
            Some(serde_json::Value::String(s)) => s != "none",
            Some(serde_json::Value::Object(_)) => true, // Specific tool choice
            None => true, // Default to auto
            _ => true,
        };

        if should_inject {
            let tools_json = serde_json::to_string(tools).unwrap_or_default();
            let tool_prompt = format!(
                "\nYou have access to the following tools:\n{}\n\nTo use a tool, you MUST use this exact syntax:\n[run tool_name arguments]\n\nExamples:\n[run list_files {{\"path\": \".\"}}]\n[run read_file {{\"path\": \"README.md\"}}]\n[run write_file {{\"path\": \"test.txt\", \"content\": \"Hello world\"}}]\n[run run_shell {{\"command\": \"ls -la\"}}]\n\nCRITICAL: Always use [run tool_name {{...}}] format. Never output raw commands.",
                tools_json
            );
            
            if let Some(sp) = &mut system_prompt_override {
                sp.push_str(&tool_prompt);
            } else {
                 system_prompt_override = Some(format!("You are a helpful assistant.\n{}", tool_prompt));
            }
        }
    }
    */

    // Prepend System Prompt
    if let Some(prompt) = system_prompt_override {
        indigo_messages.push(ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String(prompt),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for m in req.messages {
        let mut text_content = String::new();

        if let Some(s) = m.content.as_str() {
            text_content = s.to_string();
        } else if let Some(arr) = m.content.as_array() {
            for part in arr {
                if let Some(t) = part.get("type").and_then(|v| v.as_str()) {
                    if t == "text" {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            text_content.push_str(text);
                        }
                    } else if t == "image_url" {
                        if let Some(url) = part
                            .get("image_url")
                            .and_then(|v| v.get("url"))
                            .and_then(|v| v.as_str())
                        {
                            if url.starts_with("data:image") {
                                let parts: Vec<&str> = url.split(",").collect();
                                if parts.len() > 1 {
                                    if let Ok(bytes) = BASE64_STANDARD.decode(parts[1]) {
                                        image_data = Some(bytes);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle OpenAI native tool messages (role: tool, with tool_call_id)
        // Some clients send it as a separate field in the message object
        // but our OpenAIMessage struct needs to reflect that.
        // For now, we try to extract it from the raw JSON if present.
        
        let tool_calls = if let Some(tc) = m.tool_calls {
            Some(serde_json::to_value(tc).unwrap_or(serde_json::Value::Null))
        } else {
            None
        };

        // Note: We need to see if the request JSON has tool_call_id (for role: tool)
        // Since OpenAIMessage is deserialized from req.messages, let's assume it might have it.
        // (Adding tool_call_id to OpenAIMessage struct is safer)

        indigo_messages.push(ChatMessage {
            role: m.role,
            content: serde_json::Value::String(text_content),
            tool_calls,
            tool_call_id: m.tool_call_id,
        });
    }

    let prompt_payload = serde_json::to_string(&indigo_messages).unwrap_or_default();
    let model_name = resolved_model.clone();
    let id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let stream_req = req.stream.unwrap_or(false);

    // 1. Select Node based on model
    let node_address = if !model_name.is_empty() {
        state
            .get_next_node_for_model(&model_name)
            .or_else(|| state.get_next_node())
    } else {
        state.get_next_node()
    };

    let node_address = match node_address {
        Some(addr) => addr,
        None => {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                Json(OpenAIErrorResponse::new("No available nodes found", Some("no_nodes".into()))),
            ).into_response();
        }
    };

// 2. Prepare Request
    // Get tools to include in request
    let tools = {
        let registry = state.tool_registry.read().unwrap();
        registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
    };

    let grpc_req = GrpcInferenceRequest {
        prompt: prompt_payload,
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature.unwrap_or(0.7),
        image_data: image_data.unwrap_or_default(),
        stop: vec![],
        model_name: model_name.clone(),
        tools: tools,
    };

    // 3. Routing (Stdio vs gRPC)
    if node_address == "stdio://local" || !grpc_req.image_data.is_empty() {
        // Use Sidecar
        let mut sidecar_guard = state.sidecar.lock().await;
        if let Some(handle) = sidecar_guard.as_mut() {
            let req_bytes = grpc_req.encode_to_vec();
            let len_bytes = (req_bytes.len() as u32).to_be_bytes();

            if handle.stdin.write_all(&len_bytes).await.is_err()
                || handle.stdin.write_all(&req_bytes).await.is_err()
                || handle.stdin.flush().await.is_err()
            {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(OpenAIErrorResponse::new("Sidecar IO Error", Some("sidecar_io_error".into()))),
                ).into_response();
            }

            if stream_req {
                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let stdout_handle = &mut handle.stdout;
                let id_clone = id.clone();
                let model_clone = model_name.clone();

                let mut parser = ToolParser::new();
                let mut tool_emitted = false;

                loop {
                    let mut len_buf = [0u8; 4];
                    if stdout_handle.read_exact(&mut len_buf).await.is_err() {
                        break;
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    let mut msg_buf = vec![0u8; len];
                    if stdout_handle.read_exact(&mut msg_buf).await.is_err() {
                        break;
                    }

                    if let Ok(resp) = GrpcInferenceResponse::decode(std::io::Cursor::new(msg_buf)) {
                        if resp.status == 1 {
                             // Flush parser
                             if let Some(s) = parser.flush() {
                                let event = Event::default().json_data(OpenAIStreamResponse {
                                    id: id_clone.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model_clone.clone(),
                                    choices: vec![StreamChoice {
                                        index: 0,
                                        delta: Delta { content: Some(s), role: None, tool_calls: None },
                                        finish_reason: None,
                                    }],
}).map_err(axum::Error::new);
                                let _ = tx.send(event);
                            }

                            let event = Event::default()
                                .json_data(OpenAIStreamResponse {
                                    id: id_clone.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model_clone.clone(),
                                    choices: vec![StreamChoice {
                                        index: 0,
                                        delta: Delta {
                                            content: None,
                                            role: None,
                                            tool_calls: None,
                                        },
                                        finish_reason: Some(if tool_emitted { "tool_calls".to_string() } else { "stop".to_string() }),
                                    }],
                                })
                                .map_err(axum::Error::new);
                            let _ = tx.send(event);
                            break;
                        }
                        if !resp.token.is_empty() {
                            let (text, tool) = parser.push(&resp.token);
                            
                            if let Some(t) = text {
                                let event = Event::default()
                                    .json_data(OpenAIStreamResponse {
                                        id: id_clone.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_clone.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta {
                                                content: Some(t),
                                                role: None,
                                                tool_calls: None,
                                            },
                                            finish_reason: None,
                                        }],
                                    })
                                    .map_err(axum::Error::new);
                                let _ = tx.send(event);
                            }
                            
                            if let Some(tc) = tool {
                                tool_emitted = true;
                                let tc_clone = tc.clone(); // Clone for later use
                                let event = Event::default()
                                    .json_data(OpenAIStreamResponse {
                                        id: id_clone.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_clone.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta {
                                                content: None,
                                                role: None,
                                                tool_calls: Some(vec![tc]),
                                            },
                                            finish_reason: None,
                                        }],
                                    })
                                    .map_err(axum::Error::new);
                                let _ = tx.send(event);
                                
                                // Execute the tool using tool registry and executor
                                let tool_output = {
                                    let tool_def = {
                                        let registry = state.tool_registry.read().unwrap();
                                        registry.list_tools().into_iter().find(|t| t.name == tc_clone.function.name).cloned()
                                    };
                                    
                                    if let Some(td) = tool_def {
                                        let mut executor = state.tool_executor.write().await;
                                        let args: serde_json::Value = serde_json::from_str(&tc_clone.function.arguments).unwrap_or_default();
                                        match executor.execute_tool(&td, &args).await {
                                            Ok(res) => res,
                                            Err(e) => format!("Error executing tool: {}", e),
                                        }
                                    } else {
                                        format!("Tool '{}' not found", tc_clone.function.name)
                                    }
                                };
                                
                                // Stream tool execution result
                                let result_event = Event::default()
                                    .json_data(OpenAIStreamResponse {
                                        id: id_clone.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_clone.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta {
                                                content: Some(format!("\n\n[Agent Output]: {}\n", tool_output)),
                                                role: None,
                                                tool_calls: None,
                                            },
                                            finish_reason: None,
                                        }],
                                    })
                                    .map_err(axum::Error::new);
                                let _ = tx.send(result_event);
                            }
                        }
                    }
                }
                let _ = tx.send(Ok(Event::default().data("[DONE]")));
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
                return Sse::new(stream)
                    .keep_alive(axum::response::sse::KeepAlive::default())
                    .into_response();
            } else {
                let mut full_content = String::new();
                loop {
                    let mut len_buf = [0u8; 4];
                    if handle.stdout.read_exact(&mut len_buf).await.is_err() {
                        break;
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    let mut msg_buf = vec![0u8; len];
                    if handle.stdout.read_exact(&mut msg_buf).await.is_err() {
                        break;
                    }

                    if let Ok(resp) = GrpcInferenceResponse::decode(std::io::Cursor::new(msg_buf)) {
                        if resp.status == 1 {
                            break;
                        }
                        full_content.push_str(&resp.token);
                    }
                }

                let (final_content, tool_calls, finish_reason) = ToolParser::parse_static(&full_content);

                let response = OpenAIResponse {
                    id,
                    object: "chat.completion".to_string(),
                    created,
                    model: model_name,
                    choices: vec![Choice {
                        index: 0,
                        message: OpenAIMessage {
                            role: "assistant".to_string(),
                            content: if final_content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(final_content) },
                            tool_calls,
                            tool_call_id: None,
                        },
                        finish_reason,
                    }],
                    usage: Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                };
                return Json(response).into_response();
            }
        } else {
             return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenAIErrorResponse::new("Sidecar not running but requested", Some("sidecar_error".into()))),
            ).into_response();
        }
    }

    // 4. Connect to Node (gRPC)
    let mut client = match InferenceServiceClient::connect(node_address).await {
        Ok(c) => c,
        Err(e) => {
             return (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(OpenAIErrorResponse::new(format!("Failed to connect to node: {}", e), Some("connection_error".into()))),
            ).into_response();
        }
    };

    if stream_req {
        let stream: Pin<Box<dyn Stream<Item = Result<Event, axum::Error>> + Send>> =
            Box::pin(async_stream::try_stream! {
                match client.run_inference(Request::new(grpc_req)).await {
                    Ok(resp) => {
                        let mut grpc_stream = resp.into_inner();
                        let mut parser = ToolParser::new();
                        let mut tool_emitted = false;
                        let mut tool_index = 0;

                        while let Some(Ok(item)) = grpc_stream.next().await {
                            if item.status == 1 {
                                // Flush parser
                                if let Some(s) = parser.flush() {
                                     yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: Some(s), role: None, tool_calls: None },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }

                                 yield Event::default().json_data(OpenAIStreamResponse {
                                    id: id.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model_name.clone(),
                                    choices: vec![StreamChoice {
                                        index: 0,
                                        delta: Delta { content: None, role: None, tool_calls: None },
                                        finish_reason: Some(if tool_emitted { "tool_calls".to_string() } else { "stop".to_string() }),
                                    }],
                                }).map_err(axum::Error::new)?;
                                break;
                            }

                            if !item.token.is_empty() {
                                let (text, tool) = parser.push(&item.token);
                                
                                if let Some(t) = text {
                                    yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: Some(t), role: None, tool_calls: None },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }
                                
                                if let Some(mut tc) = tool {
                                    tc.index = tool_index;
                                    tool_index += 1;
                                    tool_emitted = true;
                                    yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta { content: None, role: None, tool_calls: Some(vec![tc]) },
                                            finish_reason: None,
                                        }],
                                    }).map_err(axum::Error::new)?;
                                }
                            }
                        }
                    }
                    Err(e) => {
                         eprintln!("gRPC Stream Error: {}", e);
                    }
                }

                yield Event::default().data("[DONE]");
            });

        Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::default())
            .into_response()
    } else {
        // Non-streaming: Collect all tokens
        match client.run_inference(Request::new(grpc_req)).await {
            Ok(resp) => {
                let mut grpc_stream = resp.into_inner();
                let mut full_content = String::new();
                while let Some(Ok(item)) = grpc_stream.next().await {
                    if item.status == 1 {
                        break;
                    } // Done
                    full_content.push_str(&item.token);
                }

                let (final_content, tool_calls, finish_reason) = ToolParser::parse_static(&full_content);

                let response = OpenAIResponse {
                    id,
                    object: "chat.completion".to_string(),
                    created,
                    model: model_name,
                    choices: vec![Choice {
                        index: 0,
                        message: OpenAIMessage {
                            role: "assistant".to_string(),
                            content: if final_content.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(final_content) },
                            tool_calls,
                            tool_call_id: None,
                        },
                        finish_reason,
                    }],
                    usage: Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                };
                Json(response).into_response()
            }
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(OpenAIErrorResponse::new(format!("Inference failed: {}", e), Some("inference_error".into()))),
            ).into_response(),
        }
    }
}

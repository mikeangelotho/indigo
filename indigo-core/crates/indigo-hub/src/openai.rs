use crate::tool_executor::ToolExecutor;
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
use std::sync::Arc;
use std::time::SystemTime;
use tokio_stream::StreamExt;
use tonic::Request;

use indigo_common::{
    inference::{
        inference_service_client::InferenceServiceClient, InferenceRequest as GrpcInferenceRequest,
        InferenceResponse as GrpcInferenceResponse,
    },
    ChatMessage,
};

use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{convert_common_tool_to_protobuf, AppState};

// Agentic loop configuration
const MAX_TOOL_ITERATIONS: usize = 10;

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
    #[serde(default)]
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
    #[serde(default)]
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
    #[serde(default)]
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
    pub tool_executor: Arc<tokio::sync::RwLock<ToolExecutor>>,
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

        // Try multiple parsing strategies for streaming tokens
        let (text, tool_call) = Self::try_extract_tool_call(&self.buffer);

        // 1. Check if we might have a complete JSON object
        let trimmed = self.buffer.trim();
        if trimmed.starts_with("{") && (trimmed.ends_with("}") || self.buffer.len() > 200) {
            // Try to extract valid JSON from buffer
            if let Some(tc) = tool_call {
                self.buffer.clear();
                let cleaned = text.map(|t| Self::strip_template_tokens(&t));
                return (cleaned.filter(|s| !s.is_empty()), Some(tc));
            } else {
                // Extract text before potential JSON
                if let Some(txt) = text {
                    self.buffer.clear();
                    let cleaned = Self::strip_template_tokens(&txt);
                    return (
                        if cleaned.is_empty() {
                            None
                        } else {
                            Some(cleaned)
                        },
                        None,
                    );
                }
            }
        }

        // 2. Check for natural language intent when buffer is substantial
        /* Disabled: Aggressive NL parsing causes false positives
        if self.buffer.len() > 50 && !self.buffer.contains("{") {
            if let Some(nl_tool) = Self::try_natural_language_extraction(&self.buffer) {
                eprintln!(
                    "TOOL_PARSER: Natural language tool detected: {}",
                    nl_tool.function.name
                );
                self.buffer.clear();
                return (None, Some(nl_tool));
            }
        }
        */

        (None, None)
    }

    /// Try to extract tool calls from a buffer that might contain mixed text and JSON
    fn try_extract_tool_call(buffer: &str) -> (Option<String>, Option<ToolCall>) {
        use regex::Regex;

        // Look for JSON-like patterns
        let re = Regex::new(r"(?s)\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}").unwrap();

        if let Some(mat) = re.find(buffer) {
            let json_str = mat.as_str();

            // Try to parse the extracted JSON directly
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
                if let Some(tool_call) = Self::try_parse_tool_formats(&json_value) {
                    eprintln!(
                        "TOOL_PARSER: Tool call detected: {} via format detection",
                        tool_call.function.name
                    );
                    // Extract text before the JSON as content
                    let prefix = &buffer[..mat.start()];
                    let content = if !prefix.trim().is_empty() {
                        Some(prefix.trim().to_string())
                    } else {
                        None
                    };
                    return (content, Some(tool_call));
                }
            }

            // Fallback: Try to fix unquoted keys (common with some models)
            // e.g. {function_name: glob, ...} -> {"function_name": "glob", ...}
            // This is a naive regex fix but catches common cases
            let re_keys = Regex::new(r"(\s*)(\w+)(\s*):").unwrap();
            let fixed_json = re_keys.replace_all(json_str, r#"$1"$2"$3:"#);

            // Also need to quote string values if they aren't quoted?
            // That's much harder to do safely with regex.
            // Let's rely on the prompt instructions for values, but keys are the main issue.

            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&fixed_json) {
                if let Some(tool_call) = Self::try_parse_tool_formats(&json_value) {
                    eprintln!(
                        "TOOL_PARSER: Tool call detected (relaxed JSON): {} via format detection",
                        tool_call.function.name
                    );
                    let prefix = &buffer[..mat.start()];
                    let content = if !prefix.trim().is_empty() {
                        Some(prefix.trim().to_string())
                    } else {
                        None
                    };
                    return (content, Some(tool_call));
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
            // Strip chat template tokens that leaked through
            let cleaned = Self::strip_template_tokens(&s);
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        }
    }

    /// Strip known chat template tokens from streamed text
    fn strip_template_tokens(s: &str) -> String {
        let mut out = s.to_string();
        for token in &[
            "<|im_end|>",
            "<|im_start|>",
            "<|eot_id|>",
            "<end_of_turn>",
            "<|endoftext|>",
            "<|end|>",
        ] {
            out = out.replace(token, "");
        }
        out.trim().to_string()
    }

    pub fn parse_static(content: &str) -> (String, Option<Vec<ToolCall>>, String) {
        use regex::Regex;

        // Try multiple parsing strategies in order of preference

        // 1. Try to find any JSON-like objects in the text
        let re = Regex::new(r"(?s)\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}").unwrap();
        let mut tool_calls = Vec::new();
        let last_end = 0;
        let mut final_content = content.to_string();

        for mat in re.find_iter(content) {
            let json_str = mat.as_str();

            // Try to parse the extracted block as JSON
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
                // Try different tool call formats
                if let Some(tool_call) = Self::try_parse_tool_formats(&json_value) {
                    tool_calls.push(tool_call);
                    // Remove the tool call from the content
                    if mat.start() >= last_end {
                        final_content = format!(
                            "{}{}",
                            &content[..mat.start()].trim(),
                            &content[mat.end()..].trim()
                        );
                    }
                }
            }
        }

        if !tool_calls.is_empty() {
            return (
                Self::strip_template_tokens(final_content.trim()),
                Some(tool_calls),
                "tool_calls".to_string(),
            );
        }

        (
            Self::strip_template_tokens(content),
            None,
            "stop".to_string(),
        )
    }

    fn try_parse_tool_formats(json_value: &serde_json::Value) -> Option<ToolCall> {
        // Try to parse Indigo format: {"function_name": "tool_name", "arguments": {...}}
        if let Some(function_name) = json_value.get("function_name") {
            if let Some(args) = json_value.get("arguments") {
                let name = function_name
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| function_name.to_string());
                let arguments = if args.is_string() {
                    args.as_str().unwrap_or("{}").to_string()
                } else {
                    args.to_string()
                };

                return Some(ToolCall {
                    index: 0,
                    id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                    r#type: "function".to_string(),
                    function: FunctionCall { name, arguments },
                });
            }
        }

        // Try to parse OpenAI standard format: {"name": "tool_name", "arguments": {...}}
        // NOTE: Must check this BEFORE Anthropic format since both use "name" key
        if json_value.get("function_name").is_none() {
            if let Some(name) = json_value.get("name") {
                if let Some(args) = json_value.get("arguments") {
                    let tool_name = name
                        .as_str()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| name.to_string());
                    let arguments = if args.is_string() {
                        args.as_str().unwrap_or("{}").to_string()
                    } else {
                        args.to_string()
                    };

                    return Some(ToolCall {
                        index: 0,
                        id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                        r#type: "function".to_string(),
                        function: FunctionCall {
                            name: tool_name,
                            arguments,
                        },
                    });
                }
            }
        }

        // Try to parse Anthropic format: {"name": "...", "input": {...}}
        if let Some(name) = json_value.get("name") {
            if let Some(input) = json_value.get("input") {
                let tool_name = name
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| name.to_string());
                let arguments = if input.is_string() {
                    input.as_str().unwrap_or("{}").to_string()
                } else {
                    input.to_string()
                };

                return Some(ToolCall {
                    index: 0,
                    id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: tool_name,
                        arguments,
                    },
                });
            }
        }

        None
    }

    /// Extract arguments from various JSON field names
    fn extract_arguments_from_json(json_value: &serde_json::Value) -> String {
        if let Some(args_obj) = json_value.get("arguments") {
            if args_obj.is_string() {
                args_obj.as_str().unwrap_or("{}").to_string()
            } else {
                args_obj.to_string()
            }
        } else if let Some(args_obj) = json_value.get("arguments_json") {
            args_obj.as_str().unwrap_or("{}").to_string()
        } else {
            "{}".to_string()
        }
    }

    /// Try to extract tool calls from natural language
    fn try_natural_language_extraction(content: &str) -> Option<ToolCall> {
        let content_lower = content.to_lowercase();

        // Common patterns for different tools
        if let Some((_tool_name, args)) = Self::extract_read_file_intent(&content_lower) {
            return Some(Self::create_tool_call("read_file", args));
        }

        if let Some((_tool_name, args)) = Self::extract_write_file_intent(&content_lower) {
            return Some(Self::create_tool_call("write_file", args));
        }

        if let Some((_tool_name, args)) = Self::extract_shell_intent(&content_lower) {
            return Some(Self::create_tool_call("run_shell", args));
        }

        if let Some((_tool_name, args)) = Self::extract_list_files_intent(&content_lower) {
            return Some(Self::create_tool_call("list_files", args));
        }

        None
    }

    /// Extract read file intent from natural language
    fn extract_read_file_intent(content: &str) -> Option<(String, serde_json::Value)> {
        let patterns = [
            "read\\s+(?:the\\s+)?file\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
            "open\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
            "show\\s+(?:me\\s+)?(?:the\\s+)?content\\s+of\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
            "what'?s?\\s+in\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(content) {
                    if let Some(path) = caps.get(1) {
                        return Some((
                            "read_file".to_string(),
                            serde_json::json!({"path": path.as_str()}),
                        ));
                    }
                }
            }
        }
        None
    }

    /// Extract write file intent from natural language  
    fn extract_write_file_intent(content: &str) -> Option<(String, serde_json::Value)> {
        // This is more complex and would require content extraction
        // For now, just detect intent without the content
        let patterns = [
            "write\\s+(?:to\\s+)?(?:the\\s+)?file\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
            "create\\s+(?:a\\s+)?file\\s+[\\\"]?([^\\s\\\"']+)[\\\"]?",
            "save\\s+(?:to\\s+)?[\\\"]?([^\\s\\\"']+)[\\\"]?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(content) {
                    if let Some(path) = caps.get(1) {
                        return Some((
                            "write_file".to_string(),
                            serde_json::json!({"path": path.as_str(), "content": ""}),
                        ));
                    }
                }
            }
        }
        None
    }

    /// Extract shell command intent from natural language
    fn extract_shell_intent(content: &str) -> Option<(String, serde_json::Value)> {
        let patterns = [
            "run\\s+[\\\"]?([^\\n\\\"]+)[\\\"]?",
            "execute\\s+[\\\"]?([^\\n\\\"]+)[\\\"]?",
            "shell\\s+[\\\"]?([^\\n\\\"]+)[\\\"]?",
            "command\\s+[\\\"]?([^\\n\\\"]+)[\\\"]?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(content) {
                    if let Some(cmd) = caps.get(1) {
                        return Some((
                            "run_shell".to_string(),
                            serde_json::json!({"command": cmd.as_str()}),
                        ));
                    }
                }
            }
        }
        None
    }

    /// Extract list files intent from natural language
    fn extract_list_files_intent(content: &str) -> Option<(String, serde_json::Value)> {
        let patterns = [
            "list\\s+(?:the\\s+)?files\\s+(?:in\\s+)?[\\\"]?([^\\s\\\"]*)[\\\"]?",
            "ls\\s+[\\\"]?([^\\s\\\"]*)[\\\"]?",
            "dir\\s+[\\\"]?([^\\s\\\"]*)[\\\"]?",
            "what\\s+files?\\s+(?:are\\s+)?(?:in\\s+)?[\\\"]?([^\\s\\\"]*)[\\\"]?",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(content) {
                    let path = caps.get(1).map(|m| m.as_str()).unwrap_or(".");
                    return Some(("list_files".to_string(), serde_json::json!({"path": path})));
                }
            }
        }
        None
    }

    /// Create a tool call from name and arguments
    fn create_tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            index: 0,
            id: format!("call_{}", uuid::Uuid::new_v4().simple()),
            r#type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        }
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

    /* Removed: Redundant simple tool instruction injection
    // Inject System Instruction for Tools
    if req.tools.is_some() {
        indigo_messages.push(ChatMessage {
            role: "system".to_string(),
            content: serde_json::Value::String("You have access to tools. To use a tool, output a JSON object with 'function_name' and 'arguments'.".to_string()),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    */

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
                Json(OpenAIErrorResponse::new(
                    format!("Failed to connect to proxy target: {}", e),
                    Some("connection_error".into()),
                )),
            )
                .into_response();
        }
    };

    // Get tools to include in request
    let tools = {
        let registry = state.tool_registry.read().unwrap();
        registry
            .list_tools()
            .into_iter()
            .map(convert_common_tool_to_protobuf)
            .collect::<Vec<_>>()
    };

    let grpc_req = GrpcInferenceRequest {
        prompt: prompt_payload,
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature.unwrap_or(0.7),
        image_data: image_data.clone().unwrap_or_default(),
        stop: vec![],
        model_name: model_name.clone(),
        tools: tools,
    };

    if stream_req {
        let stream: Pin<Box<dyn Stream<Item = Result<Event, axum::Error>> + Send>> = Box::pin(
            async_stream::try_stream! {
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

                                    // Store a copy for execution
                                    let tc_clone = tc.clone();

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

                                    // Execute tool and stream result (FIX: Add missing tool execution in proxy)
                                    eprintln!("PROXY_TOOL_EXECUTOR: Executing tool: {}", tc_clone.function.name);

                                    // Clone necessary data before async block to fix lifetime issues
                                    let tool_name = tc_clone.function.name.clone();
                                    let tool_args = tc_clone.function.arguments.clone();
                                    let tool_executor = state.tool_executor.clone();

                                    // Get tool definition before async block to avoid lifetime issues
                                    let tool_def = {
                                        let registry = state.tool_registry.read().unwrap();
                                        let available_tools: Vec<_> = registry
                                            .list_tools()
                                            .into_iter()
                                            .map(|t| t.name.clone())
                                            .collect();
                                        eprintln!("PROXY_TOOL_EXECUTOR: Available tools: {:?}", available_tools);

                                        registry
                                            .list_tools()
                                            .into_iter()
                                            .find(|t| t.name == tool_name)
                                            .cloned() // Clone the tool definition to avoid borrowing issues
                                    };

                                    let tool_output = {
                                        if let Some(td) = tool_def {
                                            eprintln!("PROXY_TOOL_EXECUTOR: Found tool definition: {} (type: {:?})", td.name, td.config);
                                            let executor = tool_executor.write().await;
                                            let args: serde_json::Value =
                                                serde_json::from_str(&tool_args).unwrap_or_default();
                                            eprintln!("PROXY_TOOL_EXECUTOR: Tool arguments: {}", args);
                                            match executor.execute_tool(&td, &args).await {
                                                Ok(res) => {
                                                    eprintln!("PROXY_TOOL_EXECUTOR: Tool executed successfully, output length: {}", res.len());
                                                    res
                                                },
                                                Err(e) => {
                                                    eprintln!("PROXY_TOOL_EXECUTOR: Tool execution failed: {}", e);
                                                    format!("Error executing tool: {}", e)
                                                }
                                            }
                                        } else {
                                            eprintln!("PROXY_TOOL_EXECUTOR: Tool '{}' not found in registry", tool_name);
                                            format!("Tool '{}' not found", tool_name)
                                        }
                                    };

                                    // Stream tool execution result
                                    yield Event::default().json_data(OpenAIStreamResponse {
                                        id: id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_name.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta {
                                                content: Some(format!("\n\n[Agent Output]: {}\n", tool_output)),
                                                role: None,
                                                tool_calls: None,
                                            },
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
            },
        );
        Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::default())
            .into_response()
    } else {
        // Proxy non-streaming: Agentic loop with tool execution
        let mut conversation_messages = indigo_messages.clone();
        let mut last_assistant_content = String::new();
        let mut last_tool_calls: Option<Vec<ToolCall>> = None;
        let mut last_finish_reason = String::new();
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > MAX_TOOL_ITERATIONS {
                eprintln!(
                    "AGENTIC_LOOP (proxy): Max iterations ({}) reached",
                    MAX_TOOL_ITERATIONS
                );
                last_finish_reason = "tool_calls".to_string();
                break;
            }

            let current_prompt = serde_json::to_string(&conversation_messages).unwrap_or_default();

            let tools = {
                let registry = state.tool_registry.read().unwrap();
                registry
                    .list_tools()
                    .into_iter()
                    .map(convert_common_tool_to_protobuf)
                    .collect::<Vec<_>>()
            };

            let current_grpc_req = GrpcInferenceRequest {
                prompt: current_prompt,
                max_tokens: req.max_tokens.unwrap_or(4096),
                temperature: req.temperature.unwrap_or(0.7),
                image_data: image_data.clone().unwrap_or_default(),
                stop: vec![],
                model_name: model_name.clone(),
                tools,
            };

            let mut client =
                match InferenceServiceClient::connect(state.target_address.clone()).await {
                    Ok(c) => c,
                    Err(e) => {
                        return (
                            axum::http::StatusCode::BAD_GATEWAY,
                            Json(OpenAIErrorResponse::new(
                                format!("Failed to connect to proxy target: {}", e),
                                Some("connection_error".into()),
                            )),
                        )
                            .into_response();
                    }
                };

            let resp = match client.run_inference(Request::new(current_grpc_req)).await {
                Ok(r) => r,
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(OpenAIErrorResponse::new(
                            format!("Inference failed: {}", e),
                            Some("inference_error".into()),
                        )),
                    )
                        .into_response();
                }
            };

            let mut grpc_stream = resp.into_inner();
            let mut full_content = String::new();
            while let Some(Ok(item)) = grpc_stream.next().await {
                if item.status == 1 {
                    break;
                }
                full_content.push_str(&item.token);
            }

            let (content, tool_calls, finish_reason) = ToolParser::parse_static(&full_content);

            last_assistant_content = content;
            last_tool_calls = tool_calls.clone();
            last_finish_reason = finish_reason;

            let Some(tc_list) = tool_calls else {
                break;
            };

            if tc_list.is_empty() {
                break;
            }

            eprintln!(
                "AGENTIC_LOOP (proxy): Iteration {}, executing {} tool call(s)",
                iteration,
                tc_list.len()
            );

            conversation_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: serde_json::Value::String(last_assistant_content.clone()),
                tool_calls: Some(serde_json::to_value(&tc_list).unwrap_or(serde_json::Value::Null)),
                tool_call_id: None,
            });

            let executor = state.tool_executor.read().await;
            for tc in &tc_list {
                let tool_result = {
                    let tool_def = {
                        let registry = state.tool_registry.read().unwrap();
                        registry
                            .list_tools()
                            .into_iter()
                            .find(|t| t.name == tc.function.name)
                            .cloned()
                    };

                    if let Some(td) = tool_def {
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                        match executor.execute_tool(&td, &args).await {
                            Ok(res) => res,
                            Err(e) => format!("Error executing tool: {}", e),
                        }
                    } else {
                        match executor.execute_tool_call(
                            &tc.function.name,
                            &serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
                        ) {
                            Ok(res) => res,
                            Err(e) => format!("Tool '{}' not found: {}", tc.function.name, e),
                        }
                    }
                };

                conversation_messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: serde_json::Value::String(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                });
            }
            drop(executor);
        }

        let response = OpenAIResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: model_name,
            choices: vec![Choice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: if last_assistant_content.is_empty() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(last_assistant_content)
                    },
                    tool_calls: last_tool_calls,
                    tool_call_id: None,
                },
                finish_reason: last_finish_reason,
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        };
        Json(response).into_response()
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
            eprintln!(
                "CHAT_COMPLETIONS: OpenAI Request routed to Agent: {} (model: {})",
                agent.name, agent.model
            );
            resolved_model = agent.model.clone();
            system_prompt_override = Some(agent.system_prompt.clone());
        }
    }

    eprintln!(
        "CHAT_COMPLETIONS: Processing request for model: {}, tools: {:?}",
        resolved_model,
        req.tools.is_some()
    );

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

    /// Get model-specific tool calling instructions
    fn get_model_specific_tool_instructions(
        model_name: &str,
        tools_json: &str,
        force_tool: bool,
    ) -> String {
        let name_lower = model_name.to_lowercase();

        let base_instructions = format!(
            "\n\nYou have access to the following tools:\n{}",
            tools_json
        );

        let force_instruction = if force_tool {
            "\nYou MUST call at least one tool in your response."
        } else {
            ""
        };

        if name_lower.contains("claude") || name_lower.contains("anthropic") {
            // Claude format - prefers <tool> format or JSON
            format!(
            "\n\nWhen using a tool, wrap your response in <tool> tags like this:\n\
            <tool>\n{{ \"name\": \"tool_name\", \"input\": {{ \"parameter\": \"value\" }} }}\n</tool>\n\
            Or you can use the OpenAI format directly.\n\
            {}{}",
            base_instructions,
            force_instruction
        )
        } else if name_lower.contains("gpt-4") || name_lower.contains("openai") {
            // GPT-4 format - standard OpenAI function calling
            format!(
            "\n\nTo use a tool, output a JSON object with the function name and arguments.\
            \nExample: {{ \"name\": \"read_file\", \"arguments\": {{ \"path\": \"file.txt\" }} }}\n\
            \nAlternative format: {{ \"function_name\": \"read_file\", \"arguments\": {{ \"path\": \"file.txt\" }} }}{}{}",
            base_instructions,
            force_instruction
        )
        } else if name_lower.contains("llama") || name_lower.contains("mistral") {
            // Open source models - more explicit instructions needed
            format!(
            "\n\nYou have access to tools. When you need to use one, output ONLY a JSON object:\n\
            {{ \"function_name\": \"tool_name\", \"arguments\": {{ \"parameter\": \"value\" }} }}\n\
            \nDo not include any explanatory text before or after the JSON.\n\
            \nIMPORTANT: Output ONLY the JSON for the tool call. ENSURE all keys and strings are enclosed in double quotes.\n\
            Correct: {{ \"function_name\": \"list_files\", ... }}\n\
            Incorrect: {{ function_name: list_files, ... }}{}{}",
            base_instructions,
            force_instruction
        )
        } else {
            // Default generic instructions for other models
            format!(
            "{}\n\nTo use a tool, output a JSON object with \"function_name\" and \"arguments\" keys.\n\
            Example: {{ \"function_name\": \"read_file\", \"arguments\": {{ \"path\": \"README.md\" }} }}\n\
            Alternative format: {{ \"name\": \"read_file\", \"arguments\": {{ \"path\": \"README.md\" }} }}{}",
            base_instructions,
            force_instruction
        )
        }
    }

    // Logic to determine which tools to show
    let (tools_json_str, tools_count) = if let Some(tools) = &req.tools {
        (
            serde_json::to_string(tools).unwrap_or_default(),
            tools.len(),
        )
    } else {
        // Fallback to Registry Tools
        let registry = state.tool_registry.read().unwrap();
        let tools: Vec<serde_json::Value> = registry
            .list_tools()
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();
        (
            serde_json::to_string(&tools).unwrap_or_default(),
            tools.len(),
        )
    };

    if tools_count > 0 {
        if should_inject_tools {
            let tool_instruction =
                get_model_specific_tool_instructions(&resolved_model, &tools_json_str, force_tool);

            if let Some(sp) = &mut system_prompt_override {
                sp.push_str(&tool_instruction);
            } else {
                system_prompt_override =
                    Some(format!("You are a helpful assistant.{}", tool_instruction));
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

            // Load format configuration
            let format_config = match std::fs::read_to_string("/etc/indigo/config.toml") {
                Ok(config_content) => {
                    if let Some(format) = config_content.lines()
                        .find(|line| line.trim().starts_with("default_tool_format"))
                        .and_then(|line| line.split_once('='))
                        .map(|(_, format)| format.trim())
                    {
                        format.parse().unwrap_or(indigo_common::ToolFormat::Indigo)
                    } else {
                        indigo_common::ToolFormat::Indigo
                    }
                } else {
                    indigo_common::ToolFormat::Indigo
                }
            } else {
                indigo_common::ToolFormat::Indigo
            };

            // Generate format-specific tool instructions
            let tool_instructions = match format_config {
                indigo_common::ToolFormat::Indigo => format!(
                    "\nYou have access to the following tools:\n{}\n\nTo use a tool, you MUST use this exact syntax:\n[run tool_name arguments]\n\nExamples:\n[run list_files {{\"path\": \".\"}}]\n[run read_file {{\"path\": \"README.md\"}}]\n[run write_file {{\"path\": \"test.txt\", \"content\": \"Hello world\"}}]\n[run run_shell {{\"command\": \"ls -la\"}}]\n\nCRITICAL: Always use [run tool_name {{...}}] format. Never output raw commands."
                ),
                indigo_common::ToolFormat::OpenCode => format!(
                    "\nYou have access to the following tools:\n{}\n\nTo use a tool, you MUST use this exact syntax:\n{{\"name\": \"tool_name\", \"arguments\": {{...}}}}\n\nExamples:\n{{\"name\": \"list_files\", \"arguments\": {{\"path\": \".\"}}}}\n{{\"name\": \"read_file\", \"arguments\": {{\"path\": \"README.md\"}}}}\n{{\"name\": \"write_file\", \"arguments\": {{\"path\": \"test.txt\", \"content\": \"Hello world\"}}}}\n{{\"name\": \"run_shell\", \"arguments\": {{\"command\": \"ls -la\"}}}}\n\nCRITICAL: Always use {{\"name\": \"tool_name\", \"arguments\": {{...}}}} format. Never output raw commands."
                ),
                _ => format!( // Default to Indigo format
                    "\nYou have access to the following tools:\n{}\n\nTo use a tool, you MUST use this exact syntax:\n[run tool_name arguments]\n\nExamples:\n[run list_files {{\"path\": \".\"}}]\n[run read_file {{\"path\": \"README.md\"}}]\n[run write_file {{\"path\": \"test.txt\", \"content\": \"Hello world\"}}]\n[run run_shell {{\"command\": \"ls -la\"}}]\n\nCRITICAL: Always use [run tool_name {{...}}] format. Never output raw commands."
                ),
            };

            let tool_prompt = format!(
                "\n{}{}",
                tools_json, tool_instructions
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
                Json(OpenAIErrorResponse::new(
                    "No available nodes found",
                    Some("no_nodes".into()),
                )),
            )
                .into_response();
        }
    };

    // 2. Prepare Request
    // Get tools to include in request
    let tools = {
        let registry = state.tool_registry.read().unwrap();
        registry
            .list_tools()
            .into_iter()
            .map(convert_common_tool_to_protobuf)
            .collect::<Vec<_>>()
    };

    let grpc_req = GrpcInferenceRequest {
        prompt: prompt_payload,
        max_tokens: req.max_tokens.unwrap_or(4096),
        temperature: req.temperature.unwrap_or(0.7),
        image_data: image_data.clone().unwrap_or_default(),
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
                    Json(OpenAIErrorResponse::new(
                        "Sidecar IO Error",
                        Some("sidecar_io_error".into()),
                    )),
                )
                    .into_response();
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
                                let event = Event::default()
                                    .json_data(OpenAIStreamResponse {
                                        id: id_clone.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        created,
                                        model: model_clone.clone(),
                                        choices: vec![StreamChoice {
                                            index: 0,
                                            delta: Delta {
                                                content: Some(s),
                                                role: None,
                                                tool_calls: None,
                                            },
                                            finish_reason: None,
                                        }],
                                    })
                                    .map_err(axum::Error::new);
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
                                        finish_reason: Some(if tool_emitted {
                                            "tool_calls".to_string()
                                        } else {
                                            "stop".to_string()
                                        }),
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
                                eprintln!(
                                    "TOOL_EXECUTOR: Executing tool: {}",
                                    tc_clone.function.name
                                );
                                let tool_output = {
                                    let tool_def = {
                                        let registry = state.tool_registry.read().unwrap();
                                        let available_tools: Vec<_> = registry
                                            .list_tools()
                                            .into_iter()
                                            .map(|t| t.name.clone())
                                            .collect();
                                        eprintln!(
                                            "TOOL_EXECUTOR: Available tools: {:?}",
                                            available_tools
                                        );
                                        registry
                                            .list_tools()
                                            .into_iter()
                                            .find(|t| t.name == tc_clone.function.name)
                                            .cloned()
                                    };

                                    if let Some(td) = tool_def {
                                        eprintln!(
                                            "TOOL_EXECUTOR: Found tool definition: {} (type: {:?})",
                                            td.name, td.config
                                        );
                                        let mut executor = state.tool_executor.write().await;
                                        let args: serde_json::Value =
                                            serde_json::from_str(&tc_clone.function.arguments)
                                                .unwrap_or_default();
                                        eprintln!("TOOL_EXECUTOR: Tool arguments: {}", args);
                                        match executor.execute_tool(&td, &args).await {
                                            Ok(res) => {
                                                eprintln!("TOOL_EXECUTOR: Tool executed successfully, output length: {}", res.len());
                                                res
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "TOOL_EXECUTOR: Tool execution failed: {}",
                                                    e
                                                );
                                                format!("Error executing tool: {}", e)
                                            }
                                        }
                                    } else {
                                        eprintln!(
                                            "TOOL_EXECUTOR: Tool '{}' not found in registry",
                                            tc_clone.function.name
                                        );
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
                                                content: Some(format!(
                                                    "\n\n[Agent Output]: {}\n",
                                                    tool_output
                                                )),
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
                // Sidecar non-streaming: Agentic loop with tool execution
                let mut conversation_messages = indigo_messages.clone();
                let mut last_assistant_content = String::new();
                let mut last_tool_calls: Option<Vec<ToolCall>> = None;
                let mut last_finish_reason = String::new();
                let mut iteration = 0;

                loop {
                    iteration += 1;
                    if iteration > MAX_TOOL_ITERATIONS {
                        eprintln!(
                            "AGENTIC_LOOP (sidecar): Max iterations ({}) reached",
                            MAX_TOOL_ITERATIONS
                        );
                        last_finish_reason = "tool_calls".to_string();
                        break;
                    }

                    // Serialize current conversation
                    let current_prompt =
                        serde_json::to_string(&conversation_messages).unwrap_or_default();

                    // Get current tool definitions
                    let tools = {
                        let registry = state.tool_registry.read().unwrap();
                        registry
                            .list_tools()
                            .into_iter()
                            .map(convert_common_tool_to_protobuf)
                            .collect::<Vec<_>>()
                    };

                    let current_grpc_req = GrpcInferenceRequest {
                        prompt: current_prompt,
                        max_tokens: req.max_tokens.unwrap_or(4096),
                        temperature: req.temperature.unwrap_or(0.7),
                        image_data: image_data.clone().unwrap_or_default(),
                        stop: vec![],
                        model_name: model_name.clone(),
                        tools,
                    };

                    // Send request to sidecar
                    let req_bytes = current_grpc_req.encode_to_vec();
                    let len_bytes = (req_bytes.len() as u32).to_be_bytes();

                    if handle.stdin.write_all(&len_bytes).await.is_err()
                        || handle.stdin.write_all(&req_bytes).await.is_err()
                        || handle.stdin.flush().await.is_err()
                    {
                        return (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            Json(OpenAIErrorResponse::new(
                                "Sidecar IO Error",
                                Some("sidecar_io_error".into()),
                            )),
                        )
                            .into_response();
                    }

                    // Read response from sidecar
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

                        if let Ok(resp) =
                            GrpcInferenceResponse::decode(std::io::Cursor::new(msg_buf))
                        {
                            if resp.status == 1 {
                                break;
                            }
                            full_content.push_str(&resp.token);
                        }
                    }

                    let (content, tool_calls, finish_reason) =
                        ToolParser::parse_static(&full_content);

                    last_assistant_content = content;
                    last_tool_calls = tool_calls.clone();
                    last_finish_reason = finish_reason;

                    // If no tool calls, we're done
                    let Some(tc_list) = tool_calls else {
                        break;
                    };

                    if tc_list.is_empty() {
                        break;
                    }

                    eprintln!(
                        "AGENTIC_LOOP (sidecar): Iteration {}, executing {} tool call(s)",
                        iteration,
                        tc_list.len()
                    );

                    // Append assistant message with tool calls
                    conversation_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: serde_json::Value::String(last_assistant_content.clone()),
                        tool_calls: Some(
                            serde_json::to_value(&tc_list).unwrap_or(serde_json::Value::Null),
                        ),
                        tool_call_id: None,
                    });

                    // Execute each tool and append results
                    let executor = state.tool_executor.read().await;
                    for tc in &tc_list {
                        let tool_result = {
                            let tool_def = {
                                let registry = state.tool_registry.read().unwrap();
                                registry
                                    .list_tools()
                                    .into_iter()
                                    .find(|t| t.name == tc.function.name)
                                    .cloned()
                            };

                            if let Some(td) = tool_def {
                                let args: serde_json::Value =
                                    serde_json::from_str(&tc.function.arguments)
                                        .unwrap_or_default();
                                match executor.execute_tool(&td, &args).await {
                                    Ok(res) => res,
                                    Err(e) => format!("Error executing tool: {}", e),
                                }
                            } else {
                                match executor.execute_tool_call(
                                    &tc.function.name,
                                    &serde_json::from_str(&tc.function.arguments)
                                        .unwrap_or_default(),
                                ) {
                                    Ok(res) => res,
                                    Err(e) => {
                                        format!("Tool '{}' not found: {}", tc.function.name, e)
                                    }
                                }
                            }
                        };

                        conversation_messages.push(ChatMessage {
                            role: "tool".to_string(),
                            content: serde_json::Value::String(tool_result),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                        });
                    }
                    drop(executor);
                }

                let response = OpenAIResponse {
                    id,
                    object: "chat.completion".to_string(),
                    created,
                    model: model_name,
                    choices: vec![Choice {
                        index: 0,
                        message: OpenAIMessage {
                            role: "assistant".to_string(),
                            content: if last_assistant_content.is_empty() {
                                serde_json::Value::Null
                            } else {
                                serde_json::Value::String(last_assistant_content)
                            },
                            tool_calls: last_tool_calls,
                            tool_call_id: None,
                        },
                        finish_reason: last_finish_reason,
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
                Json(OpenAIErrorResponse::new(
                    "Sidecar not running but requested",
                    Some("sidecar_error".into()),
                )),
            )
                .into_response();
        }
    }

    // 4. Connect to Node (gRPC)
    let mut client = match InferenceServiceClient::connect(node_address.clone()).await {
        Ok(c) => c,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(OpenAIErrorResponse::new(
                    format!("Failed to connect to node: {}", e),
                    Some("connection_error".into()),
                )),
            )
                .into_response();
        }
    };

    if stream_req {
        let stream: Pin<Box<dyn Stream<Item = Result<Event, axum::Error>> + Send>> = Box::pin(
            async_stream::try_stream! {
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
                                    let tc_clone = tc.clone();

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


                                    eprintln!(
                                        "TOOL_EXECUTOR (gRPC): Executing tool: {}",
                                        tc_clone.function.name
                                    );
                                    let tool_output = {
                                        let tool_def = {
                                            let registry = state.tool_registry.read().unwrap();
                                            registry
                                                .list_tools()
                                                .into_iter()
                                                .find(|t| t.name == tc_clone.function.name)
                                                .cloned()
                                        };

                                        if let Some(td) = tool_def {
                                            let mut executor = state.tool_executor.write().await;
                                            let args: serde_json::Value =
                                                serde_json::from_str(&tc_clone.function.arguments)
                                                    .unwrap_or_default();
                                            match executor.execute_tool(&td, &args).await {
                                                Ok(res) => res,
                                                Err(e) => format!("Error executing tool: {}", e),
                                            }
                                        } else {
                                            format!("Tool '{}' not found", tc_clone.function.name)
                                        }
                                    };

                                    // Stream tool execution result
                                    yield Event::default()
                                        .json_data(OpenAIStreamResponse {
                                            id: id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            created,
                                            model: model_name.clone(),
                                            choices: vec![StreamChoice {
                                                index: 0,
                                                delta: Delta {
                                                    content: Some(format!(
                                                        "\n\n[Agent Output]: {}\n",
                                                        tool_output
                                                    )),
                                                    role: None,
                                                    tool_calls: None,
                                                },
                                                finish_reason: None,
                                            }],
                                        })
                                        .map_err(axum::Error::new)?;
                                }
                            }
                        }
                    }
                    Err(e) => {
                         eprintln!("gRPC Stream Error: {}", e);
                    }
                }

                yield Event::default().data("[DONE]");
            },
        );

        Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::default())
            .into_response()
    } else {
        // Non-streaming: Agentic loop with tool execution
        let mut conversation_messages = indigo_messages.clone();
        let mut last_assistant_content = String::new();
        let mut last_tool_calls: Option<Vec<ToolCall>> = None;
        let mut last_finish_reason = String::new();
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > MAX_TOOL_ITERATIONS {
                eprintln!(
                    "AGENTIC_LOOP: Max iterations ({}) reached, stopping",
                    MAX_TOOL_ITERATIONS
                );
                last_finish_reason = "tool_calls".to_string();
                break;
            }

            // Serialize current conversation
            let current_prompt = serde_json::to_string(&conversation_messages).unwrap_or_default();

            // Get current tool definitions
            let tools = {
                let registry = state.tool_registry.read().unwrap();
                registry
                    .list_tools()
                    .into_iter()
                    .map(convert_common_tool_to_protobuf)
                    .collect::<Vec<_>>()
            };

            let current_grpc_req = GrpcInferenceRequest {
                prompt: current_prompt,
                max_tokens: req.max_tokens.unwrap_or(4096),
                temperature: req.temperature.unwrap_or(0.7),
                image_data: image_data.clone().unwrap_or_default(),
                stop: vec![],
                model_name: model_name.clone(),
                tools,
            };

            // Run inference
            let mut client = match InferenceServiceClient::connect(node_address.clone()).await {
                Ok(c) => c,
                Err(e) => {
                    return (
                        axum::http::StatusCode::BAD_GATEWAY,
                        Json(OpenAIErrorResponse::new(
                            format!("Failed to connect to node: {}", e),
                            Some("connection_error".into()),
                        )),
                    )
                        .into_response();
                }
            };

            let resp = match client.run_inference(Request::new(current_grpc_req)).await {
                Ok(r) => r,
                Err(e) => {
                    return (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        Json(OpenAIErrorResponse::new(
                            format!("Inference failed: {}", e),
                            Some("inference_error".into()),
                        )),
                    )
                        .into_response();
                }
            };

            let mut grpc_stream = resp.into_inner();
            let mut full_content = String::new();
            while let Some(Ok(item)) = grpc_stream.next().await {
                if item.status == 1 {
                    break;
                }
                full_content.push_str(&item.token);
            }

            let (content, tool_calls, finish_reason) = ToolParser::parse_static(&full_content);

            last_assistant_content = content;
            last_tool_calls = tool_calls.clone();
            last_finish_reason = finish_reason;

            // If no tool calls, we're done
            let Some(tc_list) = tool_calls else {
                break;
            };

            if tc_list.is_empty() {
                break;
            }

            eprintln!(
                "AGENTIC_LOOP: Iteration {}, executing {} tool call(s)",
                iteration,
                tc_list.len()
            );

            // Append assistant message with tool calls
            conversation_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: serde_json::Value::String(last_assistant_content.clone()),
                tool_calls: Some(serde_json::to_value(&tc_list).unwrap_or(serde_json::Value::Null)),
                tool_call_id: None,
            });

            // Execute each tool and append results
            let executor = state.tool_executor.read().await;
            for tc in &tc_list {
                let tool_result = {
                    let tool_def = {
                        let registry = state.tool_registry.read().unwrap();
                        registry
                            .list_tools()
                            .into_iter()
                            .find(|t| t.name == tc.function.name)
                            .cloned()
                    };

                    if let Some(td) = tool_def {
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                        match executor.execute_tool(&td, &args).await {
                            Ok(res) => res,
                            Err(e) => format!("Error executing tool: {}", e),
                        }
                    } else {
                        // Try the plugin registry directly
                        match executor.execute_tool_call(
                            &tc.function.name,
                            &serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
                        ) {
                            Ok(res) => res,
                            Err(e) => format!("Tool '{}' not found: {}", tc.function.name, e),
                        }
                    }
                };

                // Append tool result message
                conversation_messages.push(ChatMessage {
                    role: "tool".to_string(),
                    content: serde_json::Value::String(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                });
            }
            drop(executor);
        }

        let response = OpenAIResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: model_name,
            choices: vec![Choice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: if last_assistant_content.is_empty() {
                        serde_json::Value::Null
                    } else {
                        serde_json::Value::String(last_assistant_content)
                    },
                    tool_calls: last_tool_calls,
                    tool_call_id: None,
                },
                finish_reason: last_finish_reason,
            }],
            usage: Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
        };
        Json(response).into_response()
    }
}

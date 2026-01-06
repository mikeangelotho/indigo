pub mod inference {
    tonic::include_proto!("inference");
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub prompt: String,
    pub messages: Option<Vec<ChatMessage>>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub token: String,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    Queued,
    Streaming,
    Success,
    Error(String),
    ToolCall(ToolCallInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]

pub struct ToolCallInfo {
    pub function_name: String,

    pub arguments_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub model: String, // e.g., "llama3-8b"
    pub created_at: u64,
    #[serde(default = "default_status")]
    pub status: String, // "Online", "Offline"
}

fn default_status() -> String {
    "Offline".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub tool_type: ToolType,
    pub config: ToolConfig,
    pub permissions: Vec<String>,
    pub node_compatible: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolType {
    Native,
    Wasm,
    Http,
    Mcp,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolConfig {
    Wasm(WasmConfig),
    Http(HttpConfig),
    Mcp(McpConfig),
    Cli(CliConfig),
    Native(NativeConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeConfig {
    pub name: String, // e.g., "bash", "read", "glob", "grep"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub module_path: String,
    pub function_name: String,
    pub environment: std::collections::HashMap<String, String>,
    pub memory_limit: u32,
    pub timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub endpoint: String,
    pub method: String,
    pub headers: std::collections::HashMap<String, String>,
    pub auth_type: String,
    pub auth_token: String,
    pub verify_ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub server_name: String,
    pub server_type: String, // "stdio" or "http"
    pub args: Vec<String>,
    pub environment: std::collections::HashMap<String, String>,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub environment: std::collections::HashMap<String, String>,
    pub timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistryRequest {
    pub tool: ToolDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistryResponse {
    pub success: bool,
    pub message: String,
    pub tool: Option<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolExecutionRequest {
    pub server_name: String,
    pub tool_name: String,
    pub arguments_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolExecutionResponse {
    pub result: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPayload {
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpJsonResponse {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    pub finish_reason: Option<String>,
}

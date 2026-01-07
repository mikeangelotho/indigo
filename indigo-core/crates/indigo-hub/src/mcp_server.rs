use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Model Context Protocol (MCP) Server
/// Provides a standardized interface for external clients to access Indigo tools
pub struct McpServer {
    /// Tool executor from the hub
    tool_executor: Arc<std::sync::RwLock<crate::tool_executor::ToolExecutor>>,
    /// Tool registry from the hub
    tool_registry: Arc<std::sync::RwLock<crate::tool_registry::ToolRegistry>>,
    /// Configuration
    config: McpServerConfig,
}

#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub server_name: String,
    pub server_version: String,
    pub max_concurrent_requests: usize,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            server_name: "indigo-hub".to_string(),
            server_version: "0.1.0".to_string(),
            max_concurrent_requests: 100,
        }
    }
}

/// MCP Request/Response Types
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl McpError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

/// MCP Tool Definition
#[derive(Debug, Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

/// MCP Tool Call
#[derive(Debug, Deserialize)]
pub struct McpToolCall {
    pub name: String,
    pub arguments: Value,
}

/// MCP Tool Result
#[derive(Debug, Serialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { resource: McpResource },
}

#[derive(Debug, Serialize)]
pub struct McpResource {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Initialize state between client and server
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: McpClientInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub tools: Option<Value>,
    pub roots: Option<Value>,
    pub sampling: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: McpServerInfo,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolCapabilities,
    pub logging: Option<LoggingCapabilities>,
}

#[derive(Debug, Serialize)]
pub struct ToolCapabilities {
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LoggingCapabilities {
    pub level: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

impl McpServer {
    pub fn new(
        tool_executor: Arc<std::sync::RwLock<crate::tool_executor::ToolExecutor>>,
        tool_registry: Arc<std::sync::RwLock<crate::tool_registry::ToolRegistry>>,
        config: McpServerConfig,
    ) -> Self {
        Self {
            tool_executor,
            tool_registry,
            config,
        }
    }

    /// Handle MCP request and return response
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        println!("MCP Request: {} - {}", request.method, 
                 request.params.as_ref().map_or("".to_string(), |p| p.to_string()));

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "tools/list" => self.handle_tools_list(request.params).await,
            "tools/call" => self.handle_tools_call(request.params).await,
            "ping" => Ok(Value::Null),
            _ => Err(McpError::new(-32601, format!("Method not found: {}", request.method))),
        };

        match result {
            Ok(value) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(value),
                error: None,
            },
            Err(error) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(error),
            },
        }
    }

    async fn handle_initialize(&self, params: Option<Value>) -> Result<Value, McpError> {
        let _init_params: InitializeParams = params
            .ok_or_else(|| McpError::new(-32602, "Missing initialize params"))?
            .serde_from()
            .map_err(|e| McpError::new(-32602, format!("Invalid initialize params: {}", e)))?;

        println!("MCP Client connected: {} v{}", 
                 _init_params.client_info.name, 
                 _init_params.client_info.version);

        Ok(serde_json::to_value(InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: ToolCapabilities {
                    list_changed: Some(true),
                },
                logging: Some(LoggingCapabilities {
                    level: Some("info".to_string()),
                }),
            },
            server_info: McpServerInfo {
                name: self.config.server_name.clone(),
                version: self.config.server_version.clone(),
            },
        }).unwrap())
    }

    async fn handle_tools_list(&self, _params: Option<Value>) -> Result<Value, McpError> {
        let registry = self.tool_registry.read().unwrap();
        let tools = registry.list_tools();

        let mcp_tools: Vec<McpTool> = tools.into_iter().map(|tool| {
            McpTool {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                input_schema: tool.parameters.clone().unwrap_or_else(|| serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                })),
            }
        }).collect();

        Ok(serde_json::json!({
            "tools": mcp_tools
        }))
    }

    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value, McpError> {
        let call_params = params.ok_or_else(|| McpError::new(-32602, "Missing tool call params"))?;
        
        let name = call_params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::new(-32602, "Missing tool name"))?;

        let default_args = serde_json::Value::Object(serde_json::Map::new());
        let arguments = call_params.get("arguments")
            .unwrap_or(&default_args);

        println!("MCP Tool Call: {} with args: {}", name, arguments);

        // Find the tool definition
        let registry = self.tool_registry.read().unwrap();
        let tool_def = registry.list_tools()
            .into_iter()
            .find(|t| t.name == name);

        let tool_def = tool_def.ok_or_else(|| {
            McpError::new(-32602, format!("Tool '{}' not found", name))
        })?;

        // Execute the tool
        let mut executor = self.tool_executor.write().unwrap();
        let result = executor.execute_tool(&tool_def, arguments).await;

        match result {
            Ok(output) => {
                Ok(serde_json::to_value(McpToolResult {
                    content: vec![
                        McpContent::Text { text: output }
                    ],
                    is_error: Some(false),
                }).unwrap())
            }
            Err(e) => {
                Ok(serde_json::to_value(McpToolResult {
                    content: vec![
                        McpContent::Text { 
                            text: format!("Error executing tool '{}': {}", name, e) 
                        }
                    ],
                    is_error: Some(true),
                }).unwrap())
            }
        }
    }

    /// Export all tools as MCP format for external consumption
    pub async fn export_tools(&self) -> Vec<McpTool> {
        let registry = self.tool_registry.read().unwrap();
        let tools = registry.list_tools();

        tools.into_iter().map(|tool| {
            McpTool {
                name: tool.name.clone(),
                description: Some(tool.description.clone()),
                input_schema: tool.parameters.clone().unwrap_or_else(|| serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                })),
            }
        }).collect()
    }
}

/// Extension trait for JSON deserialization
trait SerdeExt {
    fn serde_from<T>(self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>;
}

impl SerdeExt for Value {
    fn serde_from<T>(self) -> Result<T, serde_json::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_value(self)
    }
}

/// Start MCP server as stdio transport (for MCP clients)
pub async fn start_stdio_server(
    tool_executor: Arc<std::sync::RwLock<crate::tool_executor::ToolExecutor>>,
    tool_registry: Arc<std::sync::RwLock<crate::tool_registry::ToolRegistry>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::select;
    use std::io::Write;

    let config = McpServerConfig::default();
    let server = Arc::new(McpServer::new(tool_executor, tool_registry, config));

    let mut stdin = BufReader::new(tokio::io::stdin());
    let mut line = String::new();

    loop {
        line.clear();
        
        select! {
            result = stdin.read_line(&mut line) => {
                match result {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            match serde_json::from_str::<McpRequest>(trimmed) {
                                Ok(request) => {
                                    let response = server.handle_request(request).await;
                                    let response_json = serde_json::to_string(&response)?;
                                    println!("{}", response_json);
                                    let _ = std::io::stdout().flush();
                                }
                                Err(e) => {
                                    let error_response = McpResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: None,
                                        result: None,
                                        error: Some(McpError::new(-32700, format!("Parse error: {}", e))),
                                    };
                                    let response_json = serde_json::to_string(&error_response)?;
                                    println!("{}", response_json);
                                    let _ = std::io::stdout().flush();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from stdin: {}", e);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}


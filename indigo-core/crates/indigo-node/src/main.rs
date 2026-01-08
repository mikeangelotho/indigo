mod llm;

use crate::llm::InferenceEngine;
use axum::{extract::State, routing::get, Json, Router};
use clap::Parser;
use glob::glob;
use indigo_common::inference::inference_service_client::InferenceServiceClient;
use indigo_common::inference::inference_service_server::{
    InferenceService, InferenceServiceServer,
};
use indigo_common::inference::{
    InferenceRequest, InferenceResponse, McpRequest, McpResponse, McpToolExecutionRequest,
    McpToolExecutionResponse, NodeRegistration, NodeToolsRequest, NodeToolsResponse, PingRequest,
    PingResponse, RegistrationResponse, ToolDefinition, ToolExecutionRequest,
    ToolExecutionResponse, ToolListRequest, ToolListResponse, ToolRegistryRequest,
    ToolRegistryResponse, ToolType, ToolUnregisterRequest, ToolUpdateRequest, ToolConfig, NativeConfig,
};
use indigo_common::{ChatMessage, McpJsonResponse, McpPayload, ToolCallInfo};
use prost::Message;
use regex::Regex;
use serde::Serialize;
use std::io::Cursor;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio_stream::Stream;
use tonic::{transport::Server, Request, Response, Status};

/// Indigo Node for distributed inference
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    model_repo: Option<String>,
    #[arg(long, default_value_t = String::from("Phi-3-mini-4k-instruct-q4.gguf"))]
    model_file: String,
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    set_hub_ip: String,

    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    set_node_ip: String,

    /// IP address to advertise to the Hub (defaults to set-node-ip if unset)
    #[arg(long)]
    advertise_ip: Option<String>,

    /// Port to run the gRPC server on
    #[arg(long, default_value_t = 50050)]
    port: u16,

    /// Unique identifier for this node
    #[arg(long, default_value = "node-1")]
    node_id: String,

    /// Number of layers to offload to GPU (CUDA)
    #[arg(long, default_value_t = 64)]
    gpu_layers: u32,

    /// Context window size for the model (tokens)
    #[arg(long, default_value_t = 32768)]
    n_ctx: u32,

    /// Batch size for token processing (tokens)
    #[arg(long, default_value_t = 1024)]
    n_batch: u32,

    /// Run in standard I/O mode (Sidecar) instead of gRPC
    #[arg(long)]
    stdio: bool,

    /// Path to multimodal projector file (e.g., mmproj-model-f16.gguf)
    #[arg(long)]
    mmproj: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MyInferenceService {
    engine: Arc<Mutex<InferenceEngine>>,
    tools: Arc<Mutex<HashMap<String, ToolDefinition>>>,
}

#[tonic::async_trait]
impl InferenceService for MyInferenceService {
    type RunInferenceStream = Pin<Box<dyn Stream<Item = Result<InferenceResponse, Status>> + Send>>;

    async fn register_tool(
        &self,
        request: Request<ToolRegistryRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
        let req = request.into_inner();
        if let Some(tool) = req.tool {
            let mut tools = self.tools.lock().unwrap();
            tools.insert(tool.name.clone(), tool.clone());
            println!("Registered tool: {}", tool.name);
            Ok(Response::new(ToolRegistryResponse {
                success: true,
                message: "Tool registered successfully".to_string(),
                tool: Some(tool),
            }))
        } else {
             Err(Status::invalid_argument("No tool provided"))
        }
    }

    async fn unregister_tool(
        &self,
        request: Request<ToolUnregisterRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
        let req = request.into_inner();
        let mut tools = self.tools.lock().unwrap();
        if tools.remove(&req.tool_id).is_some() {
             Ok(Response::new(ToolRegistryResponse {
                success: true,
                message: "Tool unregistered".to_string(),
                tool: None,
            }))
        } else {
             Err(Status::not_found("Tool not found"))
        }
    }

    async fn update_tool(
        &self,
        request: Request<ToolUpdateRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
         let req = request.into_inner();
        if let Some(tool) = req.tool {
            let mut tools = self.tools.lock().unwrap();
             if tools.contains_key(&req.tool_id) {
                 tools.insert(req.tool_id, tool.clone());
                 Ok(Response::new(ToolRegistryResponse {
                    success: true,
                    message: "Tool updated".to_string(),
                    tool: Some(tool),
                }))
             } else {
                  Err(Status::not_found("Tool not found"))
             }
        } else {
             Err(Status::invalid_argument("No tool provided"))
        }
    }

    async fn list_tools(
        &self,
        _request: Request<ToolListRequest>,
    ) -> Result<Response<ToolListResponse>, Status> {
        let tools = self.tools.lock().unwrap();
        let list = tools.values().cloned().collect();
        Ok(Response::new(ToolListResponse { tools: list }))
    }

    async fn execute_mcp_tool(
        &self,
        _request: Request<McpToolExecutionRequest>,
    ) -> Result<Response<McpToolExecutionResponse>, Status> {
        Err(Status::unimplemented(
            "Node does not support direct MCP tool execution yet",
        ))
    }

    async fn handle_mcp(
        &self,
        request: Request<McpRequest>,
    ) -> Result<Response<McpResponse>, Status> {
        let req = request.into_inner();
        println!("Received MCP message (len={})", req.json_message.len());

        // 1. Try Parse as McpPayload (Structured)
        let payload: McpPayload = match serde_json::from_str(&req.json_message) {
            Ok(p) => p,
            Err(_) => {
                // 2. Fallback: Try "prompt" or "content"
                let json: serde_json::Value = serde_json::from_str(&req.json_message)
                    .unwrap_or(serde_json::json!({ "prompt": req.json_message }));

                let prompt = json["prompt"]
                    .as_str()
                    .or_else(|| json["content"].as_str())
                    .unwrap_or(&req.json_message)
                    .to_string();

                McpPayload {
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: serde_json::Value::String(prompt),
                        tool_calls: None,
                        tool_call_id: None,
                    }],
                    tools: None,
                    max_tokens: Some(2048),
                    temperature: Some(0.2),
                }
            }
        };

        let engine_clone = self.engine.clone();

        // For MCP, blocking generation
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let tools_clone = payload.tools.clone();
        let msgs_clone = payload.messages.clone();
        let max_tok = payload.max_tokens.unwrap_or(2048);
        let temp = payload.temperature.unwrap_or(0.0);

        tokio::spawn(async move {
            let mut internal_rx = InferenceEngine::generate_stream(
                engine_clone,
                "".to_string(), // Unused if messages are present
                Some(msgs_clone),
                tools_clone,
                max_tok,
                temp,
                None, // No image for MCP yet
                None, // No stop tokens for direct MCP yet
            );
            while let Some(res) = internal_rx.recv().await {
                let _ = tx.send(res);
            }
        });

        let mut full_response = String::new();
        while let Some(res) = rx.recv().await {
            match res {
                Ok(token) => full_response.push_str(&token),
                Err(e) => {
                    return Err(Status::internal(format!("Inference error: {}", e)));
                }
            }
        }

        // 3. Heuristic: Is it a tool call?
        // We instructed the model to output {"function_name": ...}
        // Let's see if we can parse the whole response as JSON containing that
        let tool_calls =
            if let Ok(json_output) = serde_json::from_str::<serde_json::Value>(&full_response) {
                if let Some(fname) = json_output.get("function_name").and_then(|v| v.as_str()) {
                    let args = json_output
                        .get("arguments_json")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");
                    Some(vec![ToolCallInfo {
                        function_name: fname.to_string(),
                        arguments_json: args.to_string(),
                    }])
                } else {
                    None
                }
            } else {
                None
            };

        let resp_obj = McpJsonResponse {
            role: "assistant".to_string(),
            content: if tool_calls.is_some() {
                None
            } else {
                Some(full_response)
            },
            tool_calls,
            finish_reason: Some("stop".to_string()),
        };

        Ok(Response::new(McpResponse {
            json_response: serde_json::to_string(&resp_obj).unwrap(),
        }))
    }

    async fn run_inference(
        &self,
        request: Request<InferenceRequest>,
    ) -> Result<Response<Self::RunInferenceStream>, Status> {
        let req = request.into_inner();
        println!("Received inference request: {:?}", req);

        /* Check for Tool Calls (Simple Heuristic for now)
        if req.prompt.to_lowercase().contains("list files") {
            let output = async_stream::try_stream! {
                yield InferenceResponse {
                    token: "I noticed you want to list files. Activating agent...".to_string(),
                    status: 0,
                    error_message: "".to_string(),
                    tool_call: None,
                };

                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                 yield InferenceResponse {
                    token: "".to_string(),
                    status: 3, // TOOL_CALL
                    error_message: "".to_string(),
                    tool_call: Some(ToolCall {
                        index: 0,
                        id: format!("call_{}", uuid::Uuid::new_v4().simple()),
                        r#type: "function".to_string(),
                        function_name: "list_files".to_string(),
                        arguments_json: "{}".to_string(),
                    }),
                };
            };
            return Ok(Response::new(Box::pin(output)));
        }
        */

        // Real Inference via Candle
        let engine_clone = self.engine.clone();
        let max_tokens = req.max_tokens as usize;
        let temperature = req.temperature;

        // Check if the prompt is actually a JSON-serialized list of messages
        let messages_opt: Option<Vec<indigo_common::ChatMessage>> =
            serde_json::from_str(&req.prompt).ok();

        // Check for image data
        let image_data = if !req.image_data.is_empty() {
            Some(req.image_data)
        } else {
            None
        };
        
        let stop_tokens = if !req.stop.is_empty() {
            Some(req.stop)
        } else {
            None
        };

        let output = async_stream::try_stream! {
            let mut rx = InferenceEngine::generate_stream(engine_clone, req.prompt, messages_opt, None, max_tokens, temperature, image_data, stop_tokens);

            while let Some(result) = rx.recv().await {
                match result {
                    Ok(token) => {
                         yield InferenceResponse {
                            token,
                            status: 0, // STREAMING
                            error_message: "".to_string(),
                            tool_call: None,
                        };
                    }
                    Err(e) => {
                         yield InferenceResponse {
                            token: "".to_string(),
                            status: 2, // ERROR
                            error_message: e.to_string(),
                            tool_call: None,
                        };
                        break;
                    }
                }
            }

            // Final success message
            yield InferenceResponse {
                token: "".to_string(),
                status: 1, // SUCCESS
                error_message: "".to_string(),
                tool_call: None,
            };
        };

        Ok(Response::new(Box::pin(output)))
    }

    async fn register_node(
        &self,
        _request: Request<NodeRegistration>,
    ) -> Result<Response<RegistrationResponse>, Status> {
        Err(Status::unimplemented("Node does not accept registration"))
    }

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { active: true }))
    }

    async fn execute_tool(
        &self,
        request: Request<ToolExecutionRequest>,
    ) -> Result<Response<ToolExecutionResponse>, Status> {
        let req = request.into_inner();
        println!(
            "Node received tool execution: {} with args: {}",
            req.tool_name, req.arguments_json
        );

        let args: serde_json::Value = serde_json::from_str(&req.arguments_json).unwrap_or_default();

        let result = match req.tool_name.as_str() {
            "bash" | "run_shell" => {
                let cmd = args
                    .get("command")
                    .or_else(|| args.get("cmd"))
                    .or_else(|| args.get("input"))
                    .and_then(|v| v.as_str());
                if let Some(cmd) = cmd {
                    #[cfg(target_os = "windows")]
                    let output = std::process::Command::new("cmd")
                        .arg("/C")
                        .arg(cmd)
                        .output();
                    #[cfg(not(target_os = "windows"))]
                    let output = std::process::Command::new("bash")
                        .arg("-c")
                        .arg(cmd)
                        .output();

                    match output {
                        Ok(o) => {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            let stderr = String::from_utf8_lossy(&o.stderr);
                            format!("Output:\n{}\nErrors:\n{}", stdout, stderr)
                        }
                        Err(e) => format!("Failed to execute command: {}", e),
                    }
                } else {
                    "Missing 'command' argument".to_string()
                }
            }
            "read" | "read_file" => {
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    match std::fs::read_to_string(path) {
                        Ok(content) => content,
                        Err(e) => format!("Error reading file {}: {}", path, e),
                    }
                } else {
                    "Missing 'path' argument".to_string()
                }
            }
            "glob" | "list_files" => {
                let pattern = args
                    .get("pattern")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("*");
                match glob(pattern) {
                    Ok(paths) => {
                        let names: Vec<String> = paths
                            .filter_map(|entry| entry.ok().map(|p| p.display().to_string()))
                            .collect();
                        if names.is_empty() {
                            "No files found".to_string()
                        } else {
                            names.join("\n")
                        }
                    }
                    Err(e) => format!("Error globbing pattern {}: {}", pattern, e),
                }
            }
            "grep" => {
                let pattern_str = args.get("pattern").and_then(|v| v.as_str());
                let include = args
                    .get("include")
                    .or_else(|| args.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("*");

                if let Some(p_str) = pattern_str {
                    match Regex::new(p_str) {
                        Ok(re) => {
                            let mut matches = Vec::new();
                            match glob(include) {
                                Ok(paths) => {
                                    for entry in paths.filter_map(Result::ok) {
                                        if entry.is_file() {
                                            if let Ok(content) = std::fs::read_to_string(&entry) {
                                                for (i, line) in content.lines().enumerate() {
                                                    if re.is_match(line) {
                                                        matches.push(format!(
                                                            "{}:{}: {}",
                                                            entry.display(),
                                                            i + 1,
                                                            line
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if matches.is_empty() {
                                        "No matches found".to_string()
                                    } else {
                                        matches.join("\n")
                                    }
                                }
                                Err(e) => format!("Glob error: {}", e),
                            }
                        }
                        Err(e) => format!("Invalid regex: {}", e),
                    }
                } else {
                    "Missing 'pattern' argument".to_string()
                }
            }
            "write_file" => {
                let path = args.get("path").and_then(|v| v.as_str());
                let content = args.get("content").and_then(|v| v.as_str());

                if let (Some(path), Some(content)) = (path, content) {
                    match std::fs::write(path, content) {
                        Ok(_) => format!("Successfully wrote to file: {}", path),
                        Err(e) => format!("Error writing to file {}: {}", path, e),
                    }
                } else {
                    "Missing 'path' or 'content' argument for write_file".to_string()
                }
            }
            _ => format!("Unknown tool: {}", req.tool_name),
        };

        let is_error = result.starts_with("Error")
            || result.starts_with("Missing")
            || result.starts_with("Failed")
            || result.starts_with("Invalid");
        let error_msg = if is_error {
            result.clone()
        } else {
            String::new()
        };

        Ok(Response::new(ToolExecutionResponse {
            result,
            error: error_msg,
            success: !is_error,
        }))
    }

    async fn get_node_tools(
        &self,
        _request: Request<NodeToolsRequest>,
    ) -> Result<Response<NodeToolsResponse>, Status> {
        let mut tools = vec![
            ToolDefinition {
                name: "bash".to_string(),
                description: "Execute a shell command".to_string(),
                parameters_schema: r#"{"type": "object", "properties": {"command": {"type": "string", "description": "Shell command to execute"}}, "required": ["command"]}"#.to_string(),
                required: "[\"command\"]".to_string(),
                id: "bash".to_string(),
                r#type: ToolType::Native as i32,
                config: None,
                permissions: vec![],
                node_compatible: true,
                created_at: "".to_string(),
                updated_at: "".to_string(),
            },
        ToolDefinition {
                name: "grep".to_string(),
                description: "Search for a regex pattern in files".to_string(),
                parameters_schema: r#"{"type": "object", "properties": {"pattern": {"type": "string", "description": "Regex pattern"}, "include": {"type": "string", "description": "Glob pattern for files to include"}}, "required": ["pattern"]}"#.to_string(),
                required: "[\"pattern\"]".to_string(),
                id: "grep".to_string(),
                r#type: ToolType::Native as i32,
                config: Some(ToolConfig {
                    config: Some(indigo_common::inference::tool_config::Config::Native(NativeConfig {
                        name: "grep".to_string(),
                    })),
                }),
                permissions: vec![],
                node_compatible: true,
                created_at: "".to_string(),
                updated_at: "".to_string(),
            },
            ToolDefinition {
                name: "glob".to_string(),
                description: "List files matching a glob pattern".to_string(),
                parameters_schema: r#"{"type": "object", "properties": {"pattern": {"type": "string", "description": "Glob pattern (default: *)"}}, "required": ["pattern"]}"#.to_string(),
                required: "[\"pattern\"]".to_string(),
                id: "glob".to_string(),
                r#type: ToolType::Native as i32,
                config: None,
                permissions: vec![],
                node_compatible: true,
                created_at: "".to_string(),
                updated_at: "".to_string(),
            },
            ToolDefinition {
                name: "grep".to_string(),
                description: "Search for a regex pattern in files".to_string(),
                parameters_schema: r#"{"type": "object", "properties": {"pattern": {"type": "string", "description": "Regex pattern"}, "include": {"type": "string", "description": "Glob pattern for files to include"}}, "required": ["pattern"]}"#.to_string(),
                required: "[\"pattern\"]".to_string(),
                id: "grep".to_string(),
                r#type: ToolType::Native as i32,
                config: None,
                permissions: vec![],
                node_compatible: true,
                created_at: "".to_string(),
                updated_at: "".to_string(),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write content to a file".to_string(),
                parameters_schema: r#"{"type": "object", "properties": {"path": {"type": "string", "description": "File path"}, "content": {"type": "string", "description": "Content to write"}}, "required": ["path", "content"]}"#.to_string(),
                required: "[\"path\", \"content\"]".to_string(),
                id: "write_file".to_string(),
                r#type: ToolType::Native as i32,
                config: None,
                permissions: vec![],
                node_compatible: true,
                created_at: "".to_string(),
                updated_at: "".to_string(),
            },
        ];
        
        // Add registered tools
        {
            let registered = self.tools.lock().unwrap();
            for tool in registered.values() {
                tools.push(tool.clone());
            }
        }

        // Get node info from args or defaults
        let model_name = std::env::var("MODEL_NAME").unwrap_or_else(|_| "unknown".to_string());
        let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| "unknown".to_string());

        Ok(Response::new(NodeToolsResponse {
            tools,
            node_id,
            model_name,
        }))
    }
}

async fn run_stdio_mode_async(
    engine: Arc<Mutex<InferenceEngine>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    eprintln!("Starting Sidecar (Async Stdio Mode)...");
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();

    eprintln!("Sidecar is ready and listening for requests on Stdin.");

    loop {
        let mut len_buf = [0u8; 4];
        if stdin.read_exact(&mut len_buf).await.is_err() {
            eprintln!("Sidecar Stdin closed. Exiting.");
            break;
        }
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut msg_buf = vec![0u8; len];
        if stdin.read_exact(&mut msg_buf).await.is_err() {
            eprintln!("Sidecar failed to read message body. Exiting.");
            break;
        }

        eprintln!("Sidecar received request ({} bytes). Processing...", len);

        let req = InferenceRequest::decode(Cursor::new(msg_buf))?;

        let engine_clone = engine.clone();
        let max_tokens = req.max_tokens as usize;
        let temperature = req.temperature;

        let messages_opt: Option<Vec<indigo_common::ChatMessage>> =
            serde_json::from_str(&req.prompt).ok();

        let image_data = if !req.image_data.is_empty() {
            Some(req.image_data)
        } else {
            None
        };
        
        let stop_tokens = if !req.stop.is_empty() {
            Some(req.stop)
        } else {
            None
        };

        let mut internal_rx = InferenceEngine::generate_stream(
            engine_clone,
            req.prompt,
            messages_opt,
            None,
            max_tokens,
            temperature,
            image_data,
            stop_tokens,
        );

        while let Some(result) = internal_rx.recv().await {
            let resp = match result {
                Ok(token) => InferenceResponse {
                    token,
                    status: 0, // STREAMING
                    error_message: "".to_string(),
                    tool_call: None,
                },
                Err(e) => InferenceResponse {
                    token: "".to_string(),
                    status: 2, // ERROR
                    error_message: e.to_string(),
                    tool_call: None,
                },
            };

            let resp_bytes = resp.encode_to_vec();
            let resp_len = (resp_bytes.len() as u32).to_be_bytes();

            stdout.write_all(&resp_len).await?;
            stdout.write_all(&resp_bytes).await?;
            stdout.flush().await?;
        }

        // Final success
        let final_resp = InferenceResponse {
            token: "".to_string(),
            status: 1, // SUCCESS
            error_message: "".to_string(),
            tool_call: None,
        };
        let fb = final_resp.encode_to_vec();
        let fl = (fb.len() as u32).to_be_bytes();
        stdout.write_all(&fl).await?;
        stdout.write_all(&fb).await?;
        stdout.flush().await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut model_repo = args.model_repo;
    let model_file = args.model_file;

    if model_repo.is_none() && model_file == "Phi-3-mini-4k-instruct-q4.gguf" {
        // println!("No args provided. Defaulting to Microsoft Phi-3 (HF).");
        model_repo = Some("microsoft/Phi-3-mini-4k-instruct-gguf".to_string());
    }

    // 0. Initialize Inference Engine
    let engine = match InferenceEngine::new(
        model_repo.clone(),
        model_file.clone(),
        Some("./models".to_string()),
        args.gpu_layers,
        args.mmproj,
        args.n_ctx,
        args.n_batch,
    ) {
        Ok(e) => Arc::new(Mutex::new(e)),
        Err(e) => {
            eprintln!("Failed to load model: {}", e);
            return Err(e.into());
        }
    };

    if args.stdio {
        return run_stdio_mode_async(engine).await;
    }

    let hub_ip = args.set_hub_ip;
    let node_ip = args.set_node_ip;
    let advertise_ip = args.advertise_ip.unwrap_or(node_ip.clone());
    let port = args.port;
    let node_id = args.node_id;
    let addr = format!("{}:{}", node_ip, port).parse()?;
    let hub_address_format = format!("http://{}:3002", hub_ip);
    let hub_address = hub_address_format;

    // 1. Register with the Hub (Heartbeat Loop)
    println!("Connecting to Hub at {}...", hub_address);
    let register_model_name = model_repo.clone().unwrap_or(model_file.clone());
    let node_id_hb = node_id.clone();

    tokio::spawn(async move {
        let node_id = node_id_hb;
        loop {
            match InferenceServiceClient::connect(hub_address.clone()).await {
                Ok(mut client) => {
                    let req = tonic::Request::new(NodeRegistration {
                        node_id: node_id.clone(),
                        address: format!("http://{}:{}", advertise_ip, port),
                        port: port as u32,
                        model_name: register_model_name.clone(),
                    });

                    match client.register_node(req).await {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Failed to register/heartbeat: {}. Hub might be down.", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to connect to Hub: {}. Retrying in 10s...", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        }
    });

    // 2. Start the Inference Server
    let inference_service = MyInferenceService { 
        engine,
        tools: Arc::new(Mutex::new(HashMap::new())),
    };
    println!("Indigo Node listening on {}", addr);

    let http_addr = format!("{}:{}", node_ip, port + 100); // HTTP on port+100
    let http_addr_parsed: std::net::SocketAddr = http_addr.parse()?;

    let node_info = NodeInfo {
        node_id: node_id.clone(),
        model_name: model_repo.clone().unwrap_or(model_file.clone()),
        http_port: port + 100,
    };

    // Start gRPC server
    let grpc_server = Server::builder()
        .add_service(InferenceServiceServer::new(inference_service))
        .serve(addr);

    // Start HTTP server for tool endpoints
    let app = Router::new()
        .route("/v1/tools", get(list_node_tools))
        .route("/health", get(node_health))
        .with_state(node_info);

    let listener = tokio::net::TcpListener::bind(&http_addr_parsed).await?;
    println!("Node HTTP API listening on {} for tools", http_addr_parsed);

    // Run both servers
    tokio::try_join!(
        async move {
            grpc_server
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        },
        async move {
            axum::serve(listener, app)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        }
    )?;

    Ok(())
}

#[derive(Debug, Clone)]
struct NodeInfo {
    node_id: String,
    model_name: String,
    #[allow(dead_code)]
    http_port: u16,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    node_id: String,
    model_name: String,
}

async fn list_node_tools(State(_node_info): State<NodeInfo>) -> Json<serde_json::Value> {
    let tools = serde_json::json!([
        {
            "name": "list_files",
            "description": "List files in a directory",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path (default: .)"}
                }
            }
        },
        {
            "name": "read_file",
            "description": "Read content from a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path"}
                },
                "required": ["path"]
            }
        },
        {
            "name": "write_file",
            "description": "Write content to a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path"},
                    "content": {"type": "string", "description": "Content to write"}
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "run_shell",
            "description": "Execute a shell command",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Shell command to execute"}
                },
                "required": ["command"]
            }
        }
    ]);
    Json(tools)
}

async fn node_health(State(node_info): State<NodeInfo>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "online".to_string(),
        node_id: node_info.node_id,
        model_name: node_info.model_name,
    })
}

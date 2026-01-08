use crate::tool_executor::ToolExecutor;
use crate::tool_registry::{SharedToolRegistry, ToolRegistry};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::prelude::*;
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use indigo_common::inference::inference_service_client::InferenceServiceClient;
use indigo_common::inference::inference_service_server::{
    InferenceService, InferenceServiceServer,
};
use indigo_common::inference::{
    InferenceRequest as GrpcInferenceRequest, InferenceResponse as GrpcInferenceResponse,
    McpRequest, McpResponse, McpToolExecutionRequest, McpToolExecutionResponse, NodeRegistration,
    NodeToolsRequest, NodeToolsResponse, PingRequest, PingResponse, RegistrationResponse,
    ToolExecutionRequest, ToolExecutionResponse, ToolListRequest, ToolListResponse,
    ToolRegistryRequest, ToolRegistryResponse, ToolUnregisterRequest, ToolUpdateRequest,
};
use indigo_common::{
    AgentConfig, ChatMessage, InferenceRequest, InferenceResponse, MessageStatus, ToolCallInfo,
    ToolDefinition, ToolRegistryResponse as CommonToolRegistryResponse,
};
use prost::Message as ProstMessage; // Import Prost trait
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Duration;
use std::time::Instant;
use tokio_stream::Stream;
use tonic::{transport::Server, Request, Response, Status};
use tower_http::cors::{Any, CorsLayer};

use serde::{Deserialize, Serialize};

mod tool_executor;
mod tool_registry;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct NodeInfo {
    id: String,
    address: String,
    port: u32,
    mcp_port: Option<u32>,
    http_port: Option<u32>,
    model_name: String,
    is_local: bool,
    status: String, // "Online", "Offline"
    tool_capabilities: Option<Vec<serde_json::Value>>,
}

pub struct SidecarHandle {
    pub stdin: tokio::process::ChildStdin,
    pub stdout: tokio::process::ChildStdout,
}

#[derive(Clone)]
pub struct AppState {
    requests: Arc<RwLock<Vec<InferenceRequest>>>,
    nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
    agents: Arc<RwLock<HashMap<String, AgentConfig>>>,
    next_node: Arc<RwLock<usize>>,
    spawned_processes: Arc<RwLock<HashMap<String, u32>>>, // Track PIDs of spawned nodes
    sidecar: Arc<tokio::sync::Mutex<Option<SidecarHandle>>>,
    tool_registry: SharedToolRegistry,
    tool_executor: Arc<tokio::sync::RwLock<ToolExecutor>>,
}

impl AppState {
    fn get_next_node(&self) -> Option<String> {
        let nodes = match self.nodes.read() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to read nodes lock: {}", e);
                return None;
            }
        };
        
        if nodes.is_empty() {
            return None;
        }
        
        let mut next_idx = match self.next_node.write() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to write next_node lock: {}", e);
                return None;
            }
        };
        
        // Filter only Online nodes
        let active_nodes: Vec<String> = nodes
            .values()
            .filter(|n| n.status == "Online")
            .map(|n| n.address.clone())
            .collect();

        if active_nodes.is_empty() {
            return None;
        }

        let address = active_nodes[*next_idx % active_nodes.len()].clone();
        *next_idx = (*next_idx + 1) % active_nodes.len();
        Some(address)
    }

    fn get_next_node_for_model(&self, model_name: &str) -> Option<String> {
        let nodes = match self.nodes.read() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to read nodes lock: {}", e);
                return None;
            }
        };
        
        if nodes.is_empty() {
            return None;
        }

        // Filter active nodes that have the requested model
        let active_nodes: Vec<String> = nodes
            .values()
            .filter(|n| {
                if n.status != "Online" {
                    return false;
                }
                // Check Exact Match OR Pretty Match
                if n.model_name == model_name {
                    return true;
                }
                let pretty = prettify_model_name(&n.model_name);
                if pretty == model_name {
                    return true;
                }
                false
            })
            .map(|n| n.address.clone())
            .collect();

        if active_nodes.is_empty() {
            return None;
        }

        // Simple Round Robin (stateless for now, or use random)
        use rand::prelude::IndexedRandom;
        let mut rng = rand::rng();
        active_nodes.choose(&mut rng).cloned()
    }
}

#[derive(Clone)]
struct HubGrpcService {
    state: AppState,
}

#[tonic::async_trait]
impl InferenceService for HubGrpcService {
    async fn handle_mcp(
        &self,
        _request: Request<McpRequest>,
    ) -> Result<Response<McpResponse>, Status> {
        Err(Status::unimplemented(
            "Hub does not accept generic MCP requests directly via gRPC yet.",
        ))
    }

    async fn execute_tool(
        &self,
        request: Request<ToolExecutionRequest>,
    ) -> Result<Response<ToolExecutionResponse>, Status> {
        let req = request.into_inner();
        let tool_name = req.tool_name;
        let args_json = req.arguments_json;

        // Find tool in registry
        let tool_def = {
             let registry = match self.state.tool_registry.read() {
                 Ok(guard) => guard,
                 Err(e) => {
                     eprintln!("Failed to read tool registry: {}", e);
                     return Err(Status::internal("Failed to access tool registry"));
                 }
             };
             registry.list_tools().into_iter().find(|t| t.name == tool_name).cloned()
        };
        
        if let Some(tool) = tool_def {
            let args: serde_json::Value = serde_json::from_str(&args_json)
                .map_err(|e| Status::invalid_argument(format!("Invalid JSON args: {}", e)))?;
                
            let mut executor = self.state.tool_executor.write().await;
            match executor.execute_tool(&tool, &args).await {
                Ok(res) => Ok(Response::new(ToolExecutionResponse {
                    result: res,
                    success: true,
                    error: "".to_string(),
                })),
                Err(e) => Ok(Response::new(ToolExecutionResponse {
                    result: "".to_string(),
                    success: false,
                    error: e.to_string(),
                })),
            }
        } else {
            Err(Status::not_found(format!("Tool '{}' not found in registry", tool_name)))
        }
    }

    async fn get_node_tools(
        &self,
        _request: Request<NodeToolsRequest>,
    ) -> Result<Response<NodeToolsResponse>, Status> {
        let tools = {
             let registry = match self.state.tool_registry.read() {
                 Ok(guard) => guard,
                 Err(e) => {
                     eprintln!("Failed to read tool registry: {}", e);
                     return Err(Status::internal("Failed to access tool registry"));
                 }
             };
             registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect()
        };
        
        Ok(Response::new(NodeToolsResponse {
            tools,
            node_id: "indigo-hub".to_string(),
            model_name: "orchestrator".to_string(),
        }))
    }

    async fn register_node(
        &self,
        request: Request<NodeRegistration>,
    ) -> Result<Response<RegistrationResponse>, Status> {
        let client_ip = request
            .remote_addr()
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mut req = request.into_inner();

        // Auto-correct address if Node reports localhost/0.0.0.0 but is remote
        let is_remote_connection =
            client_ip != "127.0.0.1" && client_ip != "::1" && client_ip != "unknown";
        if is_remote_connection {
            if req.address.contains("127.0.0.1")
                || req.address.contains("0.0.0.0")
                || req.address.contains("localhost")
            {
                println!("Notice: Node {} reported local address ({}) but is connecting from {}. Auto-correcting...", req.node_id, req.address, client_ip);
                // Preserve the port
                let port_part = req.address.split(':').last().unwrap_or("50051");
                req.address = format!("http://{}:{}", client_ip, port_part);
            }
        }

        // 1. Check if node exists (Read Lock)
        let existing_mcp_port = {
            let nodes = match self.state.nodes.read() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to read nodes lock: {}", e);
                    return Err(Status::internal("Failed to access node registry"));
                }
            };
            nodes.get(&req.node_id).and_then(|n| n.mcp_port)
        };

        if let Some(_port) = existing_mcp_port {
            // Node exists, just update heartbeat/info
            // Query node for tool capabilities and get HTTP port first (before taking write lock)
            let node_address = req.address.clone();
            let (tool_capabilities, http_port) = tokio::join!(
                agent::query_node_tools(&node_address),
                async { Ok::<u32, String>(req.port + 100) } // HTTP port = gRPC port + 100
            );

            let tool_capabilities = match tool_capabilities {
                Ok(tools) => Some(tools),
                Err(e) => {
                    println!("Failed to query node tools: {}", e);
                    None
                }
            };

            // Clone values for the response message before moving them
            let http_port_for_msg = http_port.ok();
            let tool_capabilities_for_msg = tool_capabilities.clone();

            // Now take the write lock and update the node
            let mut nodes = match self.state.nodes.write() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to write nodes lock: {}", e);
                    return Err(Status::internal("Failed to update node registry"));
                }
            };
            // Check if we spawned this node (is_local) - re-evaluate or keep existing?
            // Assuming IP doesn't change for the same ID usually.
            let is_local = client_ip == "127.0.0.1" || client_ip == "::1";

            if let Some(node) = nodes.get_mut(&req.node_id) {
                node.address = req.address;
                node.port = req.port;
                node.model_name = req.model_name;
                node.status = "Online".to_string();
                node.is_local = is_local;
                node.http_port = http_port_for_msg;
                node.tool_capabilities = tool_capabilities;
                // mcp_port remains the same
            }

            return Ok(Response::new(RegistrationResponse {
                success: true,
                message: format!(
                    "Node heartbeat received. HTTP Port: {:?}, Tools: {:?}",
                    http_port_for_msg, tool_capabilities_for_msg
                ),
            }));
        }

        // 2. New Node: Spawn MCP Proxy (HTTP/OpenAI)
        let mcp_listener = tokio::net::TcpListener::bind("0.0.0.0:0")
            .await
            .map_err(|e| Status::internal(format!("Failed to bind MCP port: {}", e)))?;
        let mcp_port = mcp_listener
            .local_addr()
            .map_err(|e| Status::internal(e.to_string()))?
            .port();

        // Query node for tool capabilities and get HTTP port for new node BEFORE taking write lock
        let node_address = req.address.clone();
        let (tool_capabilities, http_port) = tokio::join!(
            agent::query_node_tools(&node_address),
            async { Ok::<u32, String>(req.port + 100) } // HTTP port = gRPC port + 100
        );

        let tool_capabilities = match tool_capabilities {
            Ok(tools) => Some(tools),
            Err(e) => {
                println!("Failed to query node tools: {}", e);
                None
            }
        };

        let is_local = client_ip == "127.0.0.1" || client_ip == "::1";

        println!(
            "Registering NEW node: {} at {} (IP: {}). OpenAI Proxy on port: {}",
            req.node_id, req.address, client_ip, mcp_port
        );

        let node_id_clone = req.node_id.clone();
        let tool_registry_clone = self.state.tool_registry.clone();
        let proxy_state = openai::ProxyState {
            target_address: req.address.clone(),
            model_name: req.model_name.clone(),
            tool_registry: tool_registry_clone,
        };

        tokio::spawn(async move {
            println!(
                "OpenAI/HTTP Proxy started for node {} on port {}",
                node_id_clone, mcp_port
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .allow_private_network(true);

            let app = Router::new()
                .route("/v1/chat/completions", post(openai::proxy_chat_completions))
                .route("/v1/models", get(openai::list_proxy_models))
                .layer(cors)
                .with_state(proxy_state);

            if let Err(e) = axum::serve(mcp_listener, app).await {
                eprintln!("Proxy server error for {}: {}", node_id_clone, e);
            }
        });

        // Now take the write lock and insert the node
        let mut nodes = match self.state.nodes.write() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to write nodes lock: {}", e);
                return Err(Status::internal("Failed to update node registry"));
            }
        };
        nodes.insert(
            req.node_id.clone(),
            NodeInfo {
                id: req.node_id,
                address: req.address,
                port: req.port,
                mcp_port: Some(mcp_port as u32),
                http_port: http_port.ok(),
                model_name: req.model_name,
                is_local,
                status: "Online".to_string(),
                tool_capabilities,
            },
        );

        Ok(Response::new(RegistrationResponse {
            success: true,
            message: format!("Node registered successfully. MCP Port: {}", mcp_port),
        }))
    }

    type RunInferenceStream =
        Pin<Box<dyn Stream<Item = Result<GrpcInferenceResponse, Status>> + Send>>;

    async fn run_inference(
        &self,
        request: Request<GrpcInferenceRequest>,
    ) -> Result<Response<Self::RunInferenceStream>, Status> {
        let req = request.into_inner();
        
        // 1. Pick a node
        let node_address = self.state.get_next_node()
             .ok_or_else(|| Status::unavailable("No nodes available"))?;

        if node_address == "stdio://local" || !req.image_data.is_empty() {
             return Err(Status::unimplemented("Sidecar/Image proxying via gRPC not fully supported yet. Use HTTP/WS API."));
        }

        let mut client = InferenceServiceClient::connect(node_address).await
            .map_err(|e| Status::internal(format!("Failed to connect to node: {}", e)))?;

        let response = client.run_inference(tonic::Request::new(req)).await
            .map_err(|e| Status::internal(format!("Node inference failed: {}", e)))?;

        let stream = response.into_inner();
        Ok(Response::new(Box::pin(stream) as Self::RunInferenceStream))
    }

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { active: true }))
    }

    async fn register_tool(
        &self,
        request: Request<ToolRegistryRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
        let req = request.into_inner();

        // Convert protobuf ToolDefinition to common ToolDefinition
        let tool = convert_protobuf_tool_to_common(req.tool.unwrap_or_default())
            .map_err(|e| Status::internal(format!("Failed to convert tool: {}", e)))?;

        match self
            .state
            .tool_registry
            .write()
            .unwrap()
            .register_tool(tool.clone())
        {
            Ok(()) => Ok(Response::new(ToolRegistryResponse {
                success: true,
                message: "Tool registered successfully".to_string(),
                tool: Some(convert_common_tool_to_protobuf(&tool)),
            })),
            Err(e) => Ok(Response::new(ToolRegistryResponse {
                success: false,
                message: format!("Failed to register tool: {}", e),
                tool: None,
            })),
        }
    }

    async fn unregister_tool(
        &self,
        request: Request<ToolUnregisterRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
        let req = request.into_inner();

        match self
            .state
            .tool_registry
            .write()
            .unwrap()
            .unregister_tool(&req.tool_id)
        {
            Ok(()) => Ok(Response::new(ToolRegistryResponse {
                success: true,
                message: "Tool unregistered successfully".to_string(),
                tool: None,
            })),
            Err(e) => Ok(Response::new(ToolRegistryResponse {
                success: false,
                message: format!("Failed to unregister tool: {}", e),
                tool: None,
            })),
        }
    }

    async fn update_tool(
        &self,
        request: Request<ToolUpdateRequest>,
    ) -> Result<Response<ToolRegistryResponse>, Status> {
        let req = request.into_inner();

        let tool = convert_protobuf_tool_to_common(req.tool.unwrap_or_default())
            .map_err(|e| Status::internal(format!("Failed to convert tool: {}", e)))?;

        match self
            .state
            .tool_registry
            .write()
            .unwrap()
            .update_tool(&req.tool_id, tool.clone())
        {
            Ok(()) => Ok(Response::new(ToolRegistryResponse {
                success: true,
                message: "Tool updated successfully".to_string(),
                tool: Some(convert_common_tool_to_protobuf(&tool)),
            })),
            Err(e) => Ok(Response::new(ToolRegistryResponse {
                success: false,
                message: format!("Failed to update tool: {}", e),
                tool: None,
            })),
        }
    }

    async fn list_tools(
        &self,
        request: Request<ToolListRequest>,
    ) -> Result<Response<ToolListResponse>, Status> {
        let req = request.into_inner();

        let protobuf_tools: Vec<indigo_common::inference::ToolDefinition> =
            if req.tool_type.is_empty() {
                let registry = self.state.tool_registry.read().unwrap();
                registry
                    .list_tools()
                    .into_iter()
                    .map(convert_common_tool_to_protobuf)
                    .collect()
            } else {
                // Filter by tool type
                let tool_type = match req.tool_type.as_str() {
                    "NATIVE" => Ok(indigo_common::ToolType::Native),
                    "WASM" => Ok(indigo_common::ToolType::Wasm),
                    "HTTP" => Ok(indigo_common::ToolType::Http),
                    "MCP" => Ok(indigo_common::ToolType::Mcp),
                    "CLI" => Ok(indigo_common::ToolType::Cli),
                    _ => Err("Invalid tool type"),
                };

                match tool_type {
                    Ok(t) => {
                        let registry = self.state.tool_registry.read().unwrap();
                        registry
                            .list_tools_by_type(&t)
                            .into_iter()
                            .map(convert_common_tool_to_protobuf)
                            .collect()
                    }
                    Err(_) => vec![],
                }
            };

        Ok(Response::new(ToolListResponse {
            tools: protobuf_tools,
        }))
    }

    async fn execute_mcp_tool(
        &self,
        request: Request<McpToolExecutionRequest>,
    ) -> Result<Response<McpToolExecutionResponse>, Status> {
        let req = request.into_inner();

        let server_name = req.server_name.clone();
        let tool_name = req.tool_name.clone();
        let arguments_json = req.arguments_json.clone();

        let result = {
            let mut executor = self.state.tool_executor.write().await;
            executor
                .execute_mcp_tool_direct(&server_name, &tool_name, &arguments_json)
                .await
        };

        match result {
            Ok(result) => Ok(Response::new(McpToolExecutionResponse {
                result,
                success: true,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(McpToolExecutionResponse {
                result: String::new(),
                success: false,
                error: e.to_string(),
            })),
        }
    }
}

// Helper functions to convert between protobuf and common tool definitions
fn convert_protobuf_tool_to_common(
    proto_tool: indigo_common::inference::ToolDefinition,
) -> Result<indigo_common::ToolDefinition, Box<dyn std::error::Error>> {
    use indigo_common::inference::tool_config::Config as ProtoConfig;
    use indigo_common::ToolConfig;
    use indigo_common::ToolType;

    let tool_type_enum = indigo_common::inference::ToolType::try_from(proto_tool.r#type)
        .map_err(|_| "Invalid tool type")?;

    let tool_type = match tool_type_enum {
        indigo_common::inference::ToolType::Native => ToolType::Native,
        indigo_common::inference::ToolType::Wasm => ToolType::Wasm,
        indigo_common::inference::ToolType::Http => ToolType::Http,
        indigo_common::inference::ToolType::Mcp => ToolType::Mcp,
        indigo_common::inference::ToolType::Cli => ToolType::Cli,
    };

    let parameters: serde_json::Value = serde_json::from_str(&proto_tool.parameters_schema)?;

    let config = if let Some(proto_tool_config) = proto_tool.config {
        if let Some(proto_config) = proto_tool_config.config {
            match proto_config {
                ProtoConfig::Wasm(wasm_config) => ToolConfig::Wasm(indigo_common::WasmConfig {
                    module_path: wasm_config.module_path.clone(),
                    function_name: wasm_config.function_name.clone(),
                    environment: wasm_config.environment.clone(),
                    memory_limit: wasm_config.memory_limit,
                    timeout: wasm_config.timeout,
                }),
                ProtoConfig::Http(http_config) => ToolConfig::Http(indigo_common::HttpConfig {
                    endpoint: http_config.endpoint.clone(),
                    method: http_config.method.clone(),
                    headers: http_config.headers.clone(),
                    auth_type: http_config.auth_type.clone(),
                    auth_token: http_config.auth_token.clone(),
                    verify_ssl: http_config.verify_ssl,
                }),
                ProtoConfig::Mcp(mcp_config) => ToolConfig::Mcp(indigo_common::McpConfig {
                    server_name: mcp_config.server_name.clone(),
                    server_type: mcp_config.server_type.clone(),
                    args: mcp_config.args.clone(),
                    environment: mcp_config.environment.clone(),
                    endpoint: mcp_config.endpoint.clone(),
                }),
                ProtoConfig::Cli(cli_config) => ToolConfig::Cli(indigo_common::CliConfig {
                    command: cli_config.command.clone(),
                    args: cli_config.args.clone(),
                    working_dir: cli_config.working_dir.clone(),
                    environment: cli_config.environment.clone(),
                    timeout: cli_config.timeout,
                }),
                ProtoConfig::Native(native_config) => {
                    ToolConfig::Native(indigo_common::NativeConfig {
                        name: native_config.name.clone(),
                    })
                }
            }
        } else {
            return Err("Missing tool config inner value".into());
        }
    } else {
        return Err("Missing tool config".into());
    };

    Ok(indigo_common::ToolDefinition {
        id: proto_tool.id,
        name: proto_tool.name,
        description: proto_tool.description,
        parameters,
        tool_type,
        config,
        permissions: proto_tool.permissions,
        node_compatible: proto_tool.node_compatible,
        context: indigo_common::ToolContext::Both, // Default to Both for converted tools
        created_at: proto_tool.created_at,
        updated_at: proto_tool.updated_at,
    })
}

fn convert_common_tool_to_protobuf(
    common_tool: &indigo_common::ToolDefinition,
) -> indigo_common::inference::ToolDefinition {
    use indigo_common::ToolConfig;
    use indigo_common::ToolType;

    let tool_type = match common_tool.tool_type {
        ToolType::Native => indigo_common::inference::ToolType::Native,
        ToolType::Wasm => indigo_common::inference::ToolType::Wasm,
        ToolType::Http => indigo_common::inference::ToolType::Http,
        ToolType::Mcp => indigo_common::inference::ToolType::Mcp,
        ToolType::Cli => indigo_common::inference::ToolType::Cli,
    };

    let parameters_schema = serde_json::to_string(&common_tool.parameters).unwrap_or_default();

    let config = match &common_tool.config {
        ToolConfig::Wasm(wasm) => Some(indigo_common::inference::ToolConfig {
            config: Some(indigo_common::inference::tool_config::Config::Wasm(
                indigo_common::inference::WasmConfig {
                    module_path: wasm.module_path.clone(),
                    function_name: wasm.function_name.clone(),
                    environment: wasm.environment.clone(),
                    memory_limit: wasm.memory_limit,
                    timeout: wasm.timeout,
                },
            )),
        }),
        ToolConfig::Http(http) => Some(indigo_common::inference::ToolConfig {
            config: Some(indigo_common::inference::tool_config::Config::Http(
                indigo_common::inference::HttpConfig {
                    endpoint: http.endpoint.clone(),
                    method: http.method.clone(),
                    headers: http.headers.clone(),
                    auth_type: http.auth_type.clone(),
                    auth_token: http.auth_token.clone(),
                    verify_ssl: http.verify_ssl,
                },
            )),
        }),
        ToolConfig::Mcp(mcp) => Some(indigo_common::inference::ToolConfig {
            config: Some(indigo_common::inference::tool_config::Config::Mcp(
                indigo_common::inference::McpConfig {
                    server_name: mcp.server_name.clone(),
                    server_type: mcp.server_type.clone(),
                    args: mcp.args.clone(),
                    environment: mcp.environment.clone(),
                    endpoint: mcp.endpoint.clone(),
                },
            )),
        }),
        ToolConfig::Cli(cli) => Some(indigo_common::inference::ToolConfig {
            config: Some(indigo_common::inference::tool_config::Config::Cli(
                indigo_common::inference::CliConfig {
                    command: cli.command.clone(),
                    args: cli.args.clone(),
                    working_dir: cli.working_dir.clone(),
                    environment: cli.environment.clone(),
                    timeout: cli.timeout,
                },
            )),
        }),
        ToolConfig::Native(native) => Some(indigo_common::inference::ToolConfig {
            config: Some(indigo_common::inference::tool_config::Config::Native(
                indigo_common::inference::NativeConfig {
                    name: native.name.clone(),
                },
            )),
        }),
    };

    indigo_common::inference::ToolDefinition {
        name: common_tool.name.clone(),
        description: common_tool.description.clone(),
        parameters_schema,
        required: serde_json::to_string(&serde_json::json!([])).unwrap_or_default(),
        id: common_tool.id.clone(),
        r#type: tool_type as i32,
        config,
        permissions: common_tool.permissions.clone(),
        node_compatible: common_tool.node_compatible,
        created_at: common_tool.created_at.clone(),
        updated_at: common_tool.updated_at.clone(),
    }
}

#[derive(serde::Deserialize)]
struct SpawnNodePayload {
    port: u16,
    model_repo: Option<String>,
    model_file: Option<String>,
    gpu_layers: Option<u32>,
    mmproj_file: Option<String>,
}

async fn spawn_node(
    State(state): State<AppState>,
    Json(payload): Json<SpawnNodePayload>,
) -> impl IntoResponse {
    use std::process::Command;

    let cwd = std::env::current_dir().unwrap_or_default();
    println!("Spawn request received. CWD: {:?}", cwd);

    let port = payload.port;
    let node_id = format!("local-node-{}", port);

    let model_file = payload
        .model_file
        .unwrap_or("Phi-3-mini-4k-instruct-q4.gguf".to_string());
    let gpu_layers = payload.gpu_layers.unwrap_or(0);

    println!("Offloading {} layers", gpu_layers);

    let mut args = vec![
        "--port".to_string(),
        port.to_string(),
        "--node-id".to_string(),
        node_id.clone(),
        "--model-file".to_string(),
        model_file.clone(),
        "--gpu-layers".to_string(),
        gpu_layers.to_string(),
        "--set-hub-ip".to_string(),
        "127.0.0.1".to_string(),
        "--set-node-ip".to_string(),
        "127.0.0.1".to_string(),
    ];

    // Force context size for Phi-3 to prevent hallucination loops
    if model_file.contains("Phi-3-mini-4k") {
        args.push("--n-ctx".to_string());
        args.push("4096".to_string());
    }

    if let Some(repo) = payload.model_repo {
        if !repo.is_empty() {
            args.push("--model-repo".to_string());
            args.push(repo);
        }
    }

    if let Some(mmproj) = payload.mmproj_file {
        if !mmproj.is_empty() {
            args.push("--mmproj".to_string());
            args.push(mmproj);
        }
    }

    #[cfg(target_os = "windows")]
    let binary_name = "indigo-node.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "indigo-node";

    let possible_paths = vec![
        format!("./{}", binary_name),
        format!("./target/release/{}", binary_name),
        format!("./target/debug/{}", binary_name),
        format!("./indigo-core/target/release/{}", binary_name),
        format!("./indigo-core/target/debug/{}", binary_name),
        format!("../target/release/{}", binary_name),
        format!("../target/debug/{}", binary_name),
        format!("../../target/release/{}", binary_name),
        format!("../../target/debug/{}", binary_name),
    ];

    let mut cmd_path = binary_name.to_string();
    let mut found_binary = false;

    for p in possible_paths {
        let path = std::path::Path::new(&p);
        if path.exists() && path.is_file() {
            if let Ok(abs) = std::fs::canonicalize(path) {
                let mut s = abs.to_string_lossy().into_owned();
                if s.starts_with(r"\\?\") {
                    s = s[4..].to_string();
                }
                cmd_path = s;
                found_binary = true;
            }
            break;
        }
    }

    if !found_binary {
        println!(
            "WARNING: Could not find {} in common paths. Defaulting to '{}'.",
            binary_name, binary_name
        );
    }

    println!("Spawning node from: {} on port {}", cmd_path, port);

    #[cfg(target_os = "windows")]
    let child = Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("Indigo Node")
        .arg("cmd")
        .arg("/K")
        .arg(&cmd_path)
        .args(&args)
        .spawn();

    #[cfg(not(target_os = "windows"))]
    let child = Command::new(cmd_path)
        .args(&args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn();

    match child {
        Ok(child) => {
            let pid = child.id();
            state
                .spawned_processes
                .write()
                .unwrap()
                .insert(node_id.clone(), pid);

            state.nodes.write().unwrap().insert(
                node_id.clone(),
                NodeInfo {
                    id: node_id.clone(),
                    address: format!("http://127.0.0.1:{}", port),
                    port: port as u32,
                    mcp_port: None,
                    model_name: model_file.clone(),
                    is_local: true,
                    status: "Starting".to_string(),
                    http_port: None,
                    tool_capabilities: None,
                },
            );

            Ok(Json(
                serde_json::json!({ "success": true, "pid": pid, "node_id": node_id }),
            ))
        }
        Err(e) => {
            eprintln!("Failed to spawn node: {}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to spawn node: {}", e),
            ))
        }
    }
}

mod openai;

mod agent {
    use indigo_common::inference::inference_service_client::InferenceServiceClient;
    use indigo_common::inference::NodeToolsRequest;
    use std::fs;
    use std::process::Command;

    pub async fn query_node_tools(node_address: &str) -> Result<Vec<serde_json::Value>, String> {
        println!("QUERYING TOOLS from node='{}'", node_address);

        let mut client = match InferenceServiceClient::connect(node_address.to_string()).await {
            Ok(client) => client,
            Err(e) => return Err(format!("Failed to connect to node: {}", e)),
        };

        let request = tonic::Request::new(NodeToolsRequest {});

        match client.get_node_tools(request).await {
            Ok(response) => {
                let resp = response.into_inner();
                // Convert ToolDefinition to JSON
                let tools: Vec<serde_json::Value> = resp.tools.into_iter().map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": serde_json::from_str::<serde_json::Value>(&tool.parameters_schema).unwrap_or_default(),
                        "required": serde_json::from_str::<serde_json::Value>(&tool.required).unwrap_or_default()
                    })
                }).collect();
                Ok(tools)
            }
            Err(e) => Err(format!("gRPC error: {}", e)),
        }
    }

    pub fn execute_tool(name: &str, args_json: &str) -> String {
        println!("LOCAL TOOL EXECUTION: tool='{}' args='{}'", name, args_json);
        match name {
            "list_files" => {
                let args: serde_json::Value =
                    serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

                match fs::read_dir(path) {
                    Ok(entries) => {
                        let names: Vec<String> = entries
                            .filter_map(|entry| {
                                entry.ok().and_then(|e| e.file_name().into_string().ok())
                            })
                            .collect();
                        format!("Files in {}: {:?}", path, names)
                    }
                    Err(e) => format!("Error listing files in {}: {}", path, e),
                }
            }
            "read_file" => {
                let args: serde_json::Value =
                    serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                    match fs::read_to_string(path) {
                        Ok(content) => format!("File content:\n{}", content),
                        Err(e) => format!("Error reading file {}: {}", path, e),
                    }
                } else {
                    "Missing 'path' argument for read_file".to_string()
                }
            }
            "write_file" => {
                let args: serde_json::Value =
                    serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                let path = args.get("path").and_then(|v| v.as_str());
                let content = args.get("content").and_then(|v| v.as_str());

                if let (Some(path), Some(content)) = (path, content) {
                    match fs::write(path, content) {
                        Ok(_) => format!("Successfully wrote to file: {}", path),
                        Err(e) => format!("Error writing to file {}: {}", path, e),
                    }
                } else {
                    "Missing 'path' or 'content' argument for write_file".to_string()
                }
            }
            "run_shell" => {
                let args: serde_json::Value =
                    serde_json::from_str(args_json).unwrap_or(serde_json::json!({}));
                if let Some(cmd) = args
                    .get("command")
                    .or_else(|| args.get("input"))
                    .and_then(|v| v.as_str())
                {
                    // Security warning: In production, this should be sandboxed!
                    #[cfg(target_os = "windows")]
                    let output = Command::new("cmd").arg("/C").arg(cmd).output();
                    #[cfg(not(target_os = "windows"))]
                    let output = Command::new("sh").arg("-c").arg(cmd).output();

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
            _ => format!("Unknown tool: {}", name),
        }
    }
}

async fn spawn_default_node(gpu_layers: u32) -> (Option<SidecarHandle>, Option<String>) {
    use std::process::Stdio;
    use tokio::process::Command;

    println!("Searching for Default Model to spawn...");

    let mut multimodal_model = None;
    let mut multimodal_mmproj = None;
    let mut any_model = None;

    // Recursive search
    fn find_files(
        dir: &std::path::Path,
        multimodal_model: &mut Option<String>,
        multimodal_mmproj: &mut Option<String>,
        any_model: &mut Option<String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find_files(&path, multimodal_model, multimodal_mmproj, any_model);
                } else if path.is_file() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let name_lower = name.to_lowercase();

                    if name_lower.ends_with(".gguf") {
                        // Keep track of ANY model
                        if !name_lower.contains("mmproj") && any_model.is_none() {
                            *any_model = Some(path.to_string_lossy().to_string());
                        }

                        // Check for Multimodal Pair
                        if name_lower.contains("mmproj") {
                            *multimodal_mmproj = Some(path.to_string_lossy().to_string());
                        } else if name_lower.contains("llava")
                            || name_lower.contains("vision")
                            || name_lower.contains("minicpm")
                            || name_lower.contains("qwen")
                        {
                            *multimodal_model = Some(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    let search_paths = vec![
        "./models",
        "./indigo-core/target/debug/models",
        "./indigo-core/crates/indigo-hub/models",
        "./indigo-core/crates/indigo-node/models",
    ];

    for path in search_paths {
        find_files(
            std::path::Path::new(path),
            &mut multimodal_model,
            &mut multimodal_mmproj,
            &mut any_model,
        );
    }

    // Decision Logic
    let (model_cmd, mmproj_cmd) = if multimodal_model.is_some() && multimodal_mmproj.is_some() {
        println!(
            "Found Multimodal Pair: {}",
            multimodal_model.as_ref().unwrap()
        );
        (multimodal_model.unwrap(), Some(multimodal_mmproj.unwrap()))
    } else if let Some(m) = any_model {
        println!(
            "No complete multimodal pair found. Fallback to text model: {}",
            m
        );
        (m, None)
    } else {
        println!("No models found in ./models. Sidecar disabled.");
        return (None, None);
    };

    // 2. Find Binary
    #[cfg(target_os = "windows")]
    let binary_name = "indigo-node.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "indigo-node";

    let possible_paths = vec![
        format!("./{}", binary_name),
        format!("./target/release/{}", binary_name),
        format!("./target/debug/{}", binary_name),
        format!("./indigo-core/target/release/{}", binary_name),
        format!("./indigo-core/target/debug/{}", binary_name),
    ];

    let mut cmd_path = binary_name.to_string();
    for p in possible_paths {
        if std::path::Path::new(&p).exists() {
            if let Ok(abs) = std::fs::canonicalize(&p) {
                let mut s = abs.to_string_lossy().into_owned();
                if s.starts_with(r"\\?\") {
                    s = s[4..].to_string();
                }
                cmd_path = s;
            }
            break;
        }
    }

    // 3. Spawn
    let mut command = Command::new(cmd_path);
    command
        .arg("--stdio")
        .arg("--model-file")
        .arg(&model_cmd)
        .arg("--gpu-layers")
        .arg(gpu_layers.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    if let Some(mm) = &mmproj_cmd {
        command.arg("--mmproj").arg(mm);
    }

    let child = command.spawn();

    match child {
        Ok(mut c) => {
            println!("Sidecar spawned successfully.");
            (
                Some(SidecarHandle {
                    stdin: c.stdin.take().unwrap(),
                    stdout: c.stdout.take().unwrap(),
                }),
                Some(model_cmd),
            )
        }
        Err(e) => {
            eprintln!("Failed to spawn sidecar: {}", e);
            (None, None)
        }
    }
}

// Token buffer for smooth WebSocket streaming
#[derive(Debug, Clone)]
struct TokenBuffer {
    content: String,
    last_flush: std::time::Instant,
    first_token_sent: bool,
    token_count: usize,
}

impl TokenBuffer {
    fn new() -> Self {
        Self {
            content: String::new(),
            last_flush: Instant::now(),
            first_token_sent: false,
            token_count: 0,
        }
    }
    
    fn add_token(&mut self, token: &str) {
        self.content.push_str(token);
        self.token_count += 1;
    }
    
    fn should_flush(&self, force: bool) -> bool {
        force ||
        // Immediate flush for first token to eliminate "thinking" delay
        (!self.first_token_sent && !self.content.is_empty()) ||
        // Adaptive buffering: start small, grow gradually for natural flow
        self.content.len() >= self.get_adaptive_threshold() ||
        // Natural timing: longer delays as conversation progresses
        self.last_flush.elapsed() >= self.get_adaptive_delay()
    }

    fn get_adaptive_threshold(&self) -> usize {
        match self.token_count {
            1..=5 => 5,   // Very small chunks at start for immediate response
            6..=15 => 15, // Small chunks for natural typing feel
            16..=50 => 30, // Medium chunks for steady flow
            _ => 50,       // Larger chunks for established streaming
        }
    }

    fn get_adaptive_delay(&self) -> Duration {
        match self.token_count {
            1..=3 => Duration::from_millis(50),   // Quick initial response
            4..=10 => Duration::from_millis(80),  // Natural typing pace
            11..=30 => Duration::from_millis(120), // Comfortable reading speed
            _ => Duration::from_millis(150),       // Steady streaming
        }
    }
    
    fn flush(&mut self) -> String {
        let result = self.content.clone();
        self.content.clear();
        self.last_flush = Instant::now();
        if !self.first_token_sent && !result.is_empty() {
            self.first_token_sent = true;
        }
        result
    }
    
    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Start the Hub with a local inference sidecar enabled
    #[arg(long)]
    with_sidecar: bool,
    
    /// Configure GPU layers for sidecar (default: 999)
    #[arg(long, default_value = "999")]
    sidecar_gpu_layers: u32,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();

    let (sidecar_handle, sidecar_model_path) = if args.with_sidecar {
        spawn_default_node(args.sidecar_gpu_layers).await
    } else {
        println!("Starting in Orchestrator Mode (Sidecar disabled). Use --with-sidecar to enable.");
        (None, None)
    };

    let shared_state = AppState {
        requests: Arc::new(RwLock::new(Vec::new())),
        nodes: Arc::new(RwLock::new(HashMap::new())),
        agents: Arc::new(RwLock::new(load_agents())),
        next_node: Arc::new(RwLock::new(0)),
        spawned_processes: Arc::new(RwLock::new(HashMap::new())),
        sidecar: Arc::new(tokio::sync::Mutex::new(sidecar_handle)),
        tool_registry: Arc::new(RwLock::new(ToolRegistry::new())),
        tool_executor: Arc::new(tokio::sync::RwLock::new(ToolExecutor::new())),
    };

    // Register Sidecar as a Node so Agents can see it
    if let Some(path) = sidecar_model_path {
        // Extract filename from path for display
        let name = std::path::Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        shared_state.nodes.write().unwrap().insert(
            "local-sidecar".to_string(),
            NodeInfo {
                id: "local-sidecar".to_string(),
                address: "stdio://local".to_string(), // Dummy address
                port: 0,
                mcp_port: None,
                model_name: name,
                is_local: true,
                status: "Online".to_string(),
                http_port: None,
                tool_capabilities: None,
            },
        );
    }

    let grpc_state = shared_state.clone();
    tokio::spawn(async move {
        let addr = match "0.0.0.0:3002".parse() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("Failed to parse gRPC address: {}", e);
                return;
            }
        };
        println!("Indigo Hub gRPC (Registration) listening on {}", addr);
        let service = HubGrpcService { state: grpc_state };

        if let Err(e) = Server::builder()
            .add_service(InferenceServiceServer::new(service))
            .serve(addr)
            .await
        {
            eprintln!("gRPC server error: {}", e);
        }
    });

    let health_state = shared_state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let nodes_to_check: Vec<(String, String)> = {
                let nodes = match health_state.nodes.read() {
                    Ok(guard) => guard,
                    Err(e) => {
                        eprintln!("Failed to read nodes for health check: {}", e);
                        continue;
                    }
                };
                nodes
                    .values()
                    .map(|n| (n.id.clone(), n.address.clone()))
                    .collect()
            };

            for (id, address) in nodes_to_check {
                // Skip the health check for the stdio sidecar, as it has no gRPC port
                if address == "stdio://local" {
                    continue;
                }

                let clean_addr = address.replace("http://", "http://");

                let active = match InferenceServiceClient::connect(clean_addr).await {
                    Ok(mut client) => {
                        let req = tonic::Request::new(PingRequest {});
                        match client.ping(req).await {
                            Ok(_) => true,
                            Err(_) => false,
                        }
                    }
                    Err(_) => false,
                };

                let mut nodes = match health_state.nodes.write() {
                    Ok(guard) => guard,
                    Err(e) => {
                        eprintln!("Failed to write nodes for health update: {}", e);
                        continue;
                    }
                };
                if let Some(node) = nodes.get_mut(&id) {
                    if active {
                        if node.status != "Online" {
                            println!("Node {} is back Online", id);
                            node.status = "Online".to_string();
                        }
                    } else {
                        if node.status == "Online" {
                            println!("Node {} went Offline", id);
                            node.status = "Offline".to_string();
                        } else if node.status == "Starting" {
                            node.status = "Offline".to_string();
                        }
                    }
                }
            }
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_private_network(true);

    let app = Router::new()
        .route("/", get(health_check))
        .route("/nodes", get(list_nodes).post(spawn_node))
        .route("/nodes/:id", axum::routing::delete(delete_node))
        .route("/models", get(list_models))
        .route("/agents", get(list_agents).post(create_agent))
        .route(
            "/agents/:id",
            get(get_agent).put(update_agent).delete(delete_agent_http),
        )
        .route("/inference", post(trigger_inference))
        .route("/v1/chat/completions", post(openai::chat_completions))
        .route("/v1/models", get(openai::list_openai_models))
        .route("/v1/tools", get(list_tools_http))
        .route("/v1/tools", post(register_tool_http))
        .route("/v1/tools/:id", put(update_tool_http))
        .route("/v1/tools/:id", delete(unregister_tool_http))
        .route("/v1/tools/legacy", get(list_tools_legacy))
        .route("/v1/mcp-servers", get(list_mcp_servers_http))
        .route("/v1/mcp-servers", post(register_mcp_server_http))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(shared_state);

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:3001").await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind HTTP server: {}", e);
            return;
        }
    };
    
    println!(
        "Indigo Hub HTTP listening on {}",
        listener.local_addr().map_or_else(|_| "unknown".to_string(), |addr| addr.to_string())
    );
    
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("HTTP server error: {}", e);
    }
}

fn save_agents(agents: &HashMap<String, AgentConfig>) {
    if let Ok(file) = std::fs::File::create("agents.json") {
        let _ = serde_json::to_writer_pretty(file, agents);
    }
}

fn load_agents() -> HashMap<String, AgentConfig> {
    if let Ok(file) = std::fs::File::open("agents.json") {
        if let Ok(agents) = serde_json::from_reader(file) {
            return agents;
        }
    }
    HashMap::new()
}

pub(crate) fn prettify_model_name(filename: &str) -> String {
    // 1. Remove common prefixes
    let mut name = filename.replace("models--", "");

    // 2. Remove extension
    if name.ends_with(".gguf") {
        name = name[..name.len() - 5].to_string();
    }

    // 3. Clean up User/Org names if present (e.g. "microsoft--")
    if let Some(idx) = name.find("--") {
        if idx + 2 < name.len() {
            name = name[idx + 2..].to_string();
        }
    }

    // 4. Replace separators
    name = name.replace("-", " ").replace("_", " ");

    name
}

async fn health_check(State(state): State<AppState>) -> Json<String> {
    let count = state.requests.read().unwrap().len();
    Json(format!("Indigo Hub is running. Active requests: {}", count))
}

async fn delete_node(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let mut nodes = match state.nodes.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write nodes lock for deletion: {}", e);
            return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to access node registry"));
        }
    };

    if nodes.remove(&id).is_some() {
        // Check if it was a spawned process and kill it
        let mut spawned = match state.spawned_processes.write() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to write spawned processes lock: {}", e);
                return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to access process registry"));
            }
        };
        if let Some(pid) = spawned.remove(&id) {
            println!("Killing process for node {}: PID {}", id, pid);

            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .output();
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
            }
        }

        Ok((axum::http::StatusCode::OK, "Node deleted"))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, "Node not found"))
    }
}

async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeInfo>> {
    let nodes = match state.nodes.read() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to read nodes for listing: {}", e);
            return Json(Vec::new());
        }
    };
    let mut list: Vec<NodeInfo> = nodes.values().cloned().collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    Json(list)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ModelInfo {
    name: String,
    file: String,
}

async fn list_models(State(state): State<AppState>) -> Json<Vec<ModelInfo>> {
    let mut models = Vec::new();

    let scan_dirs = vec![
        ".".to_string(),
        "./models".to_string(),
        "./indigo-core/target/debug/models".to_string(),
        "./indigo-core/crates/indigo-hub/models".to_string(),
        "./indigo-core/crates/indigo-node/models".to_string(),
    ];

    fn visit_dirs(dir: &std::path::Path, models: &mut Vec<ModelInfo>, prefix: &str) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, models, prefix);
                } else if let Some(ext) = path.extension() {
                    if ext == "gguf" {
                        let s = path.to_string_lossy().replace("\\", "/");
                        // We use the full relative path for file, so the node can find it.
                        let file_path = s.clone();

                        // Prettify ONLY the filename part for listing
                        let filename = path.file_name().unwrap_or_default().to_string_lossy();
                        models.push(ModelInfo {
                            name: prettify_model_name(&filename),
                            file: file_path,
                        });
                    }
                }
            }
        }
    }
    for dir in scan_dirs {
        visit_dirs(std::path::Path::new(&dir), &mut models, "");
    }

    let nodes = state.nodes.read().unwrap();
    for node in nodes.values() {
        if node.status == "Online" {
            models.push(ModelInfo {
                name: prettify_model_name(&node.model_name),
                file: node.model_name.clone(),
            });
        }
    }

    models.sort_by(|a, b| a.name.cmp(&b.name));
    models.dedup_by(|a, b| a.file == b.file);

    Json(models)
}

async fn list_agents(State(state): State<AppState>) -> Json<Vec<AgentConfig>> {
    let nodes = match state.nodes.read() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to read nodes for agent listing: {}", e);
            return Json(Vec::new());
        }
    };

    let available_models: HashSet<String> = nodes
        .values()
        .filter(|n| n.status == "Online")
        .map(|n| n.model_name.clone())
        .collect();

    let mut agents = match state.agents.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write agents lock: {}", e);
            return Json(Vec::new());
        }
    };
    let mut list: Vec<AgentConfig> = agents.values().cloned().collect();

    for agent in &mut list {
        if available_models.contains(&agent.model) {
            agent.status = "Online".to_string();
        } else {
            agent.status = "Offline".to_string();
        }
    }

    for agent in &list {
        if let Some(stored_agent) = agents.get_mut(&agent.id) {
            stored_agent.status = agent.status.clone();
        }
    }

    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Json(list)
}

#[derive(serde::Deserialize)]
struct CreateAgentPayload {
    name: String,
    system_prompt: String,
    model: String,
}

async fn create_agent(
    State(state): State<AppState>,
    Json(payload): Json<CreateAgentPayload>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let model_exists = {
        let nodes = match state.nodes.read() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to read nodes for agent creation: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        nodes
            .values()
            .any(|n| n.model_name == payload.model && n.status == "Online")
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let agent = AgentConfig {
        id: id.clone(),
        name: payload.name,
        system_prompt: payload.system_prompt,
        model: payload.model,
        created_at: now,
        status: if model_exists {
            "Online".into()
        } else {
            "Offline".into()
        },
    };

    // Store the agent
    {
        let mut agents = match state.agents.write() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to write agents lock for creation: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        agents.insert(id.clone(), agent.clone());
        save_agents(&agents);
    }

    Ok(Json(agent))
}

async fn get_agent(Path(id): Path<String>, State(state): State<AppState>) -> Result<Json<AgentConfig>, StatusCode> {
    let agents = match state.agents.read() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to read agents lock: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    match agents.get(&id) {
        Some(agent) => Ok(Json(agent.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_agent(
    Path(id): Path<String>, 
    State(state): State<AppState>,
    Json(payload): Json<CreateAgentPayload>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let mut agents = match state.agents.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write agents lock for update: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let is_online = {
        let nodes = match state.nodes.read() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Failed to read nodes for agent update: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        nodes
            .values()
            .any(|n| n.model_name == payload.model && n.status == "Online")
    };

    if let Some(agent) = agents.get_mut(&id) {
        agent.name = payload.name;
        agent.system_prompt = payload.system_prompt;
        agent.model = payload.model;
        agent.status = if is_online {
            "Online".into()
        } else {
            "Offline".into()
        };
        
        let updated_agent = agent.clone();
        
        // Release the lock before saving
        drop(agents);
        {
            let agents_for_save = match state.agents.read() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!("Failed to read agents for saving: {}", e);
                    return Ok(Json(updated_agent));
                }
            };
            save_agents(&agents_for_save);
        }
        
        Ok(Json(updated_agent))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_agent_http(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let mut agents = match state.agents.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write agents lock for deletion: {}", e);
            return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to access agent registry"));
        }
    };
    if agents.remove(&id).is_some() {
        save_agents(&agents);
        Ok((axum::http::StatusCode::OK, "Agent deleted"))
    } else {
        Err((axum::http::StatusCode::NOT_FOUND, "Agent not found"))
    }
}

async fn list_tools_legacy(State(state): State<AppState>) -> Json<serde_json::Value> {
    let nodes = match state.nodes.read() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to read nodes for legacy tools: {}", e);
            return Json(serde_json::json!({ "tools": [] }));
        }
    };
    let mut all_tools = Vec::new();

    // Aggregate tools from all nodes with tool capabilities
    for (node_id, node_info) in nodes.iter() {
        if node_info.status == "Online" {
            if let Some(ref tools) = node_info.tool_capabilities {
                for tool in tools {
                    let mut tool_with_source = tool.clone();
                    tool_with_source["node_id"] = serde_json::Value::String(node_id.clone());
                    tool_with_source["node_model"] =
                        serde_json::Value::String(node_info.model_name.clone());
                    all_tools.push(tool_with_source);
                }
            }
        }
    }

    // If no nodes with tool capabilities, fallback to hub-local tools
    if all_tools.is_empty() {
        all_tools = vec![
            serde_json::json!({
                "name": "list_files",
                "description": "List files in a directory (Hub local)",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Directory path (default: .)"}
                    }
                },
                "source": "hub"
            }),
            serde_json::json!({
                "name": "read_file",
                "description": "Read file content (Hub local)",
                 "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path"}
                    },
                    "required": ["path"]
                },
                "source": "hub"
            }),
            serde_json::json!({
                "name": "write_file",
                "description": "Write content to a file (Hub local)",
                 "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path"},
                        "content": {"type": "string", "description": "Content to write"}
                    },
                    "required": ["path", "content"]
                },
                "source": "hub"
            }),
            serde_json::json!({
                "name": "run_shell",
                "description": "Execute a shell command (Hub local)",
                 "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "Shell command to execute"}
                    },
                    "required": ["command"]
                },
                "source": "hub"
            }),
        ];
    }

    Json(serde_json::Value::Array(all_tools))
}

// New tool management HTTP endpoints
async fn list_tools_http(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Vec<ToolDefinition>> {
    let registry = match state.tool_registry.read() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to read tool registry for HTTP listing: {}", e);
            return Json(Vec::new());
        }
    };
    
    let all_tools = registry.list_tools();
    
    // Filter by context if specified
    let filtered_tools = if let Some(context) = params.get("context") {
        match context.as_str() {
            "web" => all_tools.into_iter()
                .filter(|t| matches!(t.context, indigo_common::ToolContext::WebChat | indigo_common::ToolContext::Both))
                .cloned()
                .collect(),
            "cli" => all_tools.into_iter()
                .filter(|t| matches!(t.context, indigo_common::ToolContext::CliInterface | indigo_common::ToolContext::Both))
                .cloned()
                .collect(),
            _ => all_tools.into_iter().cloned().collect(),
        }
    } else {
        all_tools.into_iter().cloned().collect()
    };
    
    Json(filtered_tools)
}

async fn register_tool_http(
    State(state): State<AppState>,
    Json(tool): Json<ToolDefinition>,
) -> Json<CommonToolRegistryResponse> {
    let mut registry = match state.tool_registry.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write tool registry lock for registration: {}", e);
            return Json(CommonToolRegistryResponse {
                success: false,
                message: "Failed to access tool registry".to_string(),
                tool: None,
            });
        }
    };
    
    match registry.register_tool(tool.clone())
    {
        Ok(()) => Json(CommonToolRegistryResponse {
            success: true,
            message: "Tool registered successfully".to_string(),
            tool: Some(tool),
        }),
        Err(e) => Json(CommonToolRegistryResponse {
            success: false,
            message: format!("Failed to register tool: {}", e),
            tool: None,
        }),
    }
}

async fn update_tool_http(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
    Json(tool): Json<ToolDefinition>,
) -> Json<CommonToolRegistryResponse> {
    let mut registry = match state.tool_registry.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write tool registry lock for update: {}", e);
            return Json(CommonToolRegistryResponse {
                success: false,
                message: "Failed to access tool registry".to_string(),
                tool: None,
            });
        }
    };
    
    match registry.update_tool(&tool_id, tool.clone())
    {
        Ok(()) => Json(CommonToolRegistryResponse {
            success: true,
            message: "Tool updated successfully".to_string(),
            tool: Some(tool),
        }),
        Err(e) => Json(CommonToolRegistryResponse {
            success: false,
            message: format!("Failed to update tool: {}", e),
            tool: None,
        }),
    }
}

async fn unregister_tool_http(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
) -> Json<CommonToolRegistryResponse> {
    let mut registry = match state.tool_registry.write() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to write tool registry lock for unregistration: {}", e);
            return Json(CommonToolRegistryResponse {
                success: false,
                message: "Failed to access tool registry".to_string(),
                tool: None,
            });
        }
    };
    
    match registry.unregister_tool(&tool_id)
    {
        Ok(()) => Json(CommonToolRegistryResponse {
            success: true,
            message: "Tool unregistered successfully".to_string(),
            tool: None,
        }),
        Err(e) => Json(CommonToolRegistryResponse {
            success: false,
            message: format!("Failed to unregister tool: {}", e),
            tool: None,
        }),
    }
}

#[derive(serde::Deserialize)]
struct McpServerRequest {
    server_name: String,
    server_type: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    endpoint: Option<String>,
    environment: Option<std::collections::HashMap<String, String>>,
}

async fn list_mcp_servers_http(State(state): State<AppState>) -> Json<serde_json::Value> {
    let servers = state.tool_executor.read().await.list_mcp_servers();
    Json(serde_json::json!({ "servers": servers }))
}

async fn register_mcp_server_http(
    State(state): State<AppState>,
    Json(req): Json<McpServerRequest>,
) -> Json<serde_json::Value> {
    use indigo_common::McpConfig;

    let mut args = req.args.unwrap_or_default();
    if let Some(cmd) = req.command {
        if !args.contains(&cmd) {
            args.insert(0, cmd);
        }
    }

    let config = McpConfig {
        server_name: req.server_name.clone(),
        server_type: req.server_type.clone(),
        args,
        environment: req.environment.unwrap_or_default(),
        endpoint: req.endpoint.unwrap_or_default(),
    };

    match state
        .tool_executor
        .write()
        .await
        .register_mcp_server(&config)
        .await
    {
        Ok(()) => Json(serde_json::json!({
            "success": true,
            "message": format!("MCP server '{}' registered successfully", req.server_name)
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("Failed to register MCP server: {}", e)
        })),
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    println!("Websocket connected");
    let (ws_sender, mut ws_receiver) = socket.split();
    let shared_sender = Arc::new(Mutex::new(ws_sender));

    while let Some(Ok(msg)) = ws_receiver.next().await {
        let state_clone = state.clone();
        let sender_clone = Arc::clone(&shared_sender);

        if let Message::Text(text) = msg {
            // USER MSG println!("{}", text);
            tokio::spawn(async move {
                let mut token_buffer = TokenBuffer::new();
                let mut req: InferenceRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = send_error(&sender_clone, &format!("Invalid JSON: {}", e)).await;
                        return;
                    }
                };

                // Check for image data in messages
                let mut image_data: Option<Vec<u8>> = None;
                if let Some(msgs) = &req.messages {
                    for m in msgs {
                        if let Some(arr) = m.content.as_array() {
                            for part in arr {
                                if let Some(t) = part.get("type").and_then(|v| v.as_str()) {
                                    if t == "image_url" {
                                        if let Some(url) = part
                                            .get("image_url")
                                            .and_then(|v| v.get("url"))
                                            .and_then(|v| v.as_str())
                                        {
                                            if url.starts_with("data:image") {
                                                let parts: Vec<&str> = url.split(",").collect();
                                                if parts.len() > 1 {
                                                    if let Ok(bytes) =
                                                        BASE64_STANDARD.decode(parts[1])
                                                    {
                                                        image_data = Some(bytes);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

if let Some(agent_id) = &req.agent_id {
                    let agent_info = {
                        let agents = state_clone.agents.read().unwrap();
                        agents
                            .get(agent_id)
                            .map(|a| (a.name.clone(), a.system_prompt.clone(), a.model.clone()))
                    };
                    
if let Some((name, prompt, _model)) = agent_info {
                        println!("Using agent: {}", name);

                        let mut final_prompt = prompt;

                        // Add tool instructions if tools are available
                        let tools_for_instructions = if let Some(ref tools) = req.tools {
                            tools.clone()
                        } else {
                            // Filter tools based on request context
                            let registry = state_clone.tool_registry.read().unwrap();
                            let all_tools = registry.list_tools();
                            
                            match req.context.as_deref().unwrap_or("cli") {
                                "web" => all_tools
                                    .into_iter()
                                    .filter(|t| matches!(t.context, indigo_common::ToolContext::WebChat | indigo_common::ToolContext::Both))
                                    .cloned()
                                    .collect(),
                                "cli" => all_tools
                                    .into_iter()
                                    .filter(|t| matches!(t.context, indigo_common::ToolContext::CliInterface | indigo_common::ToolContext::Both))
                                    .cloned()
                                    .collect(),
                                _ => all_tools.into_iter().cloned().collect(),
                            }
                        };

                        if !tools_for_instructions.is_empty() {
                            final_prompt.push_str("\n\nYou have access to the following tools:\n");
                            for tool in &tools_for_instructions {
                                final_prompt.push_str(&format!("- {}: {}\n", tool.name, tool.description));
                            }
                            
                            final_prompt.push_str("\nTo use a tool, you MUST use this exact format:\n");
                            final_prompt.push_str("{\"function_name\": \"tool_name\", \"arguments\": {...}}\n\n");
                            final_prompt.push_str("Examples:\n");
                            final_prompt.push_str("{\"function_name\": \"list_files\", \"arguments\": {\"path\": \".\"}}\n");
                            final_prompt.push_str("{\"function_name\": \"read_file\", \"arguments\": {\"path\": \"README.md\"}}\n");
                            final_prompt.push_str("{\"function_name\": \"write_file\", \"arguments\": {\"path\": \"test.txt\", \"content\": \"Hello world\"}}\n");
                            final_prompt.push_str("{\"function_name\": \"run_shell\", \"arguments\": {\"command\": \"ls -la\"}}\n\n");
                            final_prompt.push_str("IMPORTANT: Always use the JSON format for tool calls. Do not output raw commands.\n");
                            final_prompt.push_str("The tool call should be a standalone JSON object on its own line.");
                        }

                        let system_msg = ChatMessage {
                            role: "system".to_string(),
                            content: serde_json::Value::String(final_prompt),
                            tool_calls: None,
                            tool_call_id: None,
                        };

                        if let Some(msgs) = &mut req.messages {
                            // Prepend agent system prompt
                            msgs.insert(0, system_msg);
                        } else {
                            req.messages = Some(vec![system_msg]);
                        }
                    } else {
                        let _ = send_error(&sender_clone, "Agent not found").await;
                        return;
                    }
                }

                let prompt_payload = if let Some(msgs) = &req.messages {
                    match serde_json::to_string(msgs) {
                        Ok(s) => s,
                        Err(_) => req.prompt.clone(),
                    }
                } else {
                    req.prompt.clone()
                };

                println!("{}", prompt_payload);

// --- ROUTING LOGIC ---
                // Extract model name for multimodal case
                let model_name = if let Some(agent_id) = &req.agent_id {
                    let agents = state_clone.agents.read().unwrap();
                    agents
                        .get(agent_id)
                        .map(|a| a.model.clone())
                        .unwrap_or_default()
                } else {
                    "".to_string()
                };

                if image_data.is_some() {
                    let mut sidecar_guard = state_clone.sidecar.lock().await;
                    if let Some(handle) = sidecar_guard.as_mut() {
                        println!("Routing to Multimodal Sidecar...");

                        // Get tools to include in request
                        let tools = {
                            let registry = state_clone.tool_registry.read().unwrap();
                            registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
                        };

                        let grpc_req = GrpcInferenceRequest {
                            prompt: prompt_payload,
                            max_tokens: req.max_tokens as u32,
                            temperature: req.temperature,
                            image_data: image_data.unwrap(),
                            stop: vec![],
                            model_name,
                            tools: tools,
                        };

                        let req_bytes = grpc_req.encode_to_vec();
                        let len_bytes = (req_bytes.len() as u32).to_be_bytes();

                        if let Err(e) = handle.stdin.write_all(&len_bytes).await {
                            let _ =
                                send_error(&sender_clone, &format!("Sidecar write error: {}", e))
                                    .await;
                            return;
                        }
                        if let Err(e) = handle.stdin.write_all(&req_bytes).await {
                            let _ =
                                send_error(&sender_clone, &format!("Sidecar write error: {}", e))
                                    .await;
                            return;
                        }
                        if let Err(e) = handle.stdin.flush().await {
                            let _ =
                                send_error(&sender_clone, &format!("Sidecar flush error: {}", e))
                                    .await;
                            return;
                        }

                        // Read Response Stream
                        let mut parser = openai::ToolParser::new();
                        loop {
                            let mut len_buf = [0u8; 4];
                            if let Err(_) = handle.stdout.read_exact(&mut len_buf).await {
                                break;
                            }
                            let len = u32::from_be_bytes(len_buf) as usize;
                            let mut msg_buf = vec![0u8; len];
                            if let Err(_) = handle.stdout.read_exact(&mut msg_buf).await {
                                break;
                            }

                            if let Ok(resp) =
                                GrpcInferenceResponse::decode(std::io::Cursor::new(msg_buf))
                            {
                                if resp.status == 1 {
                                    if let Some(s) = parser.flush() {
                                        let flush_resp = InferenceResponse {
                                            token: s,
                                            status: MessageStatus::Streaming,
                                        };
                                        let _ = send_json_buffered(&sender_clone, &flush_resp, &mut token_buffer, false).await;
                                    }
                                    let success_resp = InferenceResponse {
                                        token: "".to_string(),
                                        status: MessageStatus::Success,
                                    };
                                    let _ = send_json_buffered(&sender_clone, &success_resp, &mut token_buffer, false).await;
                                    break;
                                }

                                if !resp.token.is_empty() {
                                    let (text, tool) = parser.push(&resp.token);
                                    if let Some(t) = text {
                                        let _ = send_json(
                                            &sender_clone,
                                            &InferenceResponse {
                                                token: t,
                                                status: MessageStatus::Streaming,
                                            },
                                        )
                                        .await;
                                    }
                                    if let Some(tc) = tool {
                                        // Use the same ToolExecutor as gRPC nodes for consistency
                                        let tool_def = {
                                             let registry = state_clone.tool_registry.read().unwrap();
                                             registry.list_tools().into_iter().find(|t| t.name == tc.function.name).cloned()
                                        };

                                        let output = if let Some(td) = tool_def {
                                            let mut executor = state_clone.tool_executor.write().await;
                                            let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                                            match executor.execute_tool(&td, &args).await {
                                                Ok(res) => res,
                                                Err(e) => format!("Error executing tool: {}", e),
                                            }
                                        } else {
                                             format!("Error: Tool '{}' not found", tc.function.name)
                                        };

                                        let tool_resp = InferenceResponse {
                                                token: output,
                                                status: MessageStatus::ToolCall(ToolCallInfo {
                                                    function_name: tc.function.name.clone(),
                                                    arguments_json: tc.function.arguments.clone(),
                                                }),
                                            };
                                        let _ = send_json_buffered(&sender_clone, &tool_resp, &mut token_buffer, false).await;
                                    }
                                }
                            }
                        }
                        return; // Done with Sidecar
                    } else {
                        println!("Request has image but no Sidecar running. Falling back to text-only node.");
                    }
                }

                // --- STANDARD NODE LOGIC ---

                let target_model = if let Some(agent_id) = &req.agent_id {
                    let agents = state_clone.agents.read().unwrap();
                    if let Some(agent) = agents.get(agent_id) {
                        agent.model.clone()
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                let node_address = if !target_model.is_empty() {
                    match state_clone.get_next_node_for_model(&target_model) {
                        Some(addr) => addr,
                        None => {
                            let _ = send_error(
                                &sender_clone,
                                &format!("No online nodes for model: {}", target_model),
                            )
                            .await;
                            return;
                        }
                    }
                } else {
                    match state_clone.get_next_node() {
                        Some(addr) => addr,
                        None => {
                            let _ = send_error(&sender_clone, "No nodes registered").await;
                            return;
                        }
                    }
                };

println!("Forwarding request to node at: {}", node_address);

                // Route to sidecar if address is stdio://local
                if node_address == "stdio://local" {
                    let mut sidecar_guard = state_clone.sidecar.lock().await;
                    if let Some(handle) = sidecar_guard.as_mut() {
                        println!("Routing to Sidecar via WebSocket...");

                        // Get tools to include in request
                        let tools = {
                            let registry = state_clone.tool_registry.read().unwrap();
                            registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
                        };

                        let grpc_req = GrpcInferenceRequest {
                            prompt: prompt_payload,
                            max_tokens: req.max_tokens as u32,
                            temperature: req.temperature,
                            image_data: image_data.unwrap_or_default(),
                            stop: vec![],
                            model_name: target_model,
                            tools: tools,
                        };

                        let req_bytes = grpc_req.encode_to_vec();
                        let len_bytes = (req_bytes.len() as u32).to_be_bytes();

                        if let Err(e) = handle.stdin.write_all(&len_bytes).await {
                            let _ = send_error(&sender_clone, &format!("Sidecar write error: {}", e)).await;
                            return;
                        }
                        if let Err(e) = handle.stdin.write_all(&req_bytes).await {
                            let _ = send_error(&sender_clone, &format!("Sidecar write error: {}", e)).await;
                            return;
                        }
                        if let Err(e) = handle.stdin.flush().await {
                            let _ = send_error(&sender_clone, &format!("Sidecar flush error: {}", e)).await;
                            return;
                        }

                        // Read Response Stream
                        let mut parser = openai::ToolParser::new();
                        loop {
                            let mut len_buf = [0u8; 4];
                            if let Err(_) = handle.stdout.read_exact(&mut len_buf).await {
                                break;
                            }
                            let len = u32::from_be_bytes(len_buf) as usize;
                            let mut msg_buf = vec![0u8; len];
                            if let Err(_) = handle.stdout.read_exact(&mut msg_buf).await {
                                break;
                            }

                            if let Ok(resp) = GrpcInferenceResponse::decode(std::io::Cursor::new(msg_buf)) {
                                if resp.status == 1 {
                                    if let Some(s) = parser.flush() {
                                        let flush_resp = InferenceResponse {
                                            token: s,
                                            status: MessageStatus::Streaming,
                                        };
                                        let _ = send_json_buffered(&sender_clone, &flush_resp, &mut token_buffer, false).await;
                                    }
                                    let success_resp = InferenceResponse {
                                        token: "".to_string(),
                                        status: MessageStatus::Success,
                                    };
                                    let _ = send_json_buffered(&sender_clone, &success_resp, &mut token_buffer, false).await;
                                    break;
                                }

                                if !resp.token.is_empty() {
                                    let (text, tool) = parser.push(&resp.token);
                                    if let Some(t) = text {
                                        let text_resp = InferenceResponse {
                                            token: t,
                                            status: MessageStatus::Streaming,
                                        };
                                        let _ = send_json_buffered(&sender_clone, &text_resp, &mut token_buffer, false).await;
                                    }
                                    if let Some(tc) = tool {
                                        // Use the same ToolExecutor as gRPC nodes for consistency
                                        let tool_def = {
                                             let registry = state_clone.tool_registry.read().unwrap();
                                             registry.list_tools().into_iter().find(|t| t.name == tc.function.name).cloned()
                                        };

                                        println!("SIDECAR_TOOL_EXECUTION: Executing tool '{}' via ToolExecutor", tc.function.name);
                                        let output = if let Some(td) = tool_def {
                                            let mut executor = state_clone.tool_executor.write().await;
                                            let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                                            match executor.execute_tool(&td, &args).await {
                                                Ok(res) => res,
                                                Err(e) => format!("Error executing tool: {}", e),
                                            }
                                        } else {
                                             format!("Error: Tool '{}' not found", tc.function.name)
                                        };

                                        let tool_resp = InferenceResponse {
                                                token: output,
                                                status: MessageStatus::ToolCall(ToolCallInfo {
                                                    function_name: tc.function.name.clone(),
                                                    arguments_json: tc.function.arguments.clone(),
                                                }),
                                            };
                                        let _ = send_json_buffered(&sender_clone, &tool_resp, &mut token_buffer, false).await;
                                    }
                                }
                            }
                        }
                        return; // Done with Sidecar
                    } else {
                        let _ = send_error(&sender_clone, "Sidecar not running but requested").await;
                        return;
                    }
                }

                let mut client = match InferenceServiceClient::connect(node_address.to_string())
                    .await
                {
                    Ok(client) => client,
                    Err(e) => {
                        let _ =
                            send_error(&sender_clone, &format!("Node connection failed: {}", e))
                                .await;
                        return;
                    }
                };

                // Get tools to include in request
                let tools = {
                    let registry = state_clone.tool_registry.read().unwrap();
                    registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
                };

 let grpc_req = tonic::Request::new(GrpcInferenceRequest {
                    prompt: prompt_payload,
                    max_tokens: req.max_tokens as u32,
                    temperature: req.temperature,
                    image_data: image_data.unwrap_or_default(),
                    stop: vec![],
                    model_name: target_model,
                    tools: tools,
                });

                match client.run_inference(grpc_req).await {
                    Ok(response) => {
                        let mut stream = response.into_inner();
                        let mut parser = openai::ToolParser::new();

                        while let Some(Ok(resp)) = stream.next().await {
                            let status = map_status(&resp);

                            if status == MessageStatus::Success {
                                if let Some(s) = parser.flush() {
                                    if let Err(_) = send_json(
                                        &sender_clone,
                                        &InferenceResponse {
                                            token: s,
                                            status: MessageStatus::Streaming,
                                        },
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                }
                                let _ = send_json(
                                    &sender_clone,
                                    &InferenceResponse {
                                        token: "".to_string(),
                                        status: MessageStatus::Success,
                                    },
                                )
                                .await;
                                break;
                            }

                            // Check for status-based tool call (from gRPC)
                            if let MessageStatus::ToolCall(info) = status {
                                let tool_def = {
                                     let registry = state_clone.tool_registry.read().unwrap();
                                     registry.list_tools().into_iter().find(|t| t.name == info.function_name).cloned()
                                };

                                println!("GRPC_TOOL_EXECUTION: Executing tool '{}' via ToolExecutor (status-based)", info.function_name);
                                let output = if let Some(td) = tool_def {
                                    let mut executor = state_clone.tool_executor.write().await;
                                    let args: serde_json::Value = serde_json::from_str(&info.arguments_json).unwrap_or_default();
                                    match executor.execute_tool(&td, &args).await {
                                        Ok(res) => res,
                                        Err(e) => format!("Error executing tool: {}", e),
                                    }
                                } else {
                                     format!("Error: Tool '{}' not found", info.function_name)
                                };

                                let tool_resp = InferenceResponse {
                                    token: format!("\n\n[Agent Output]: {}\n", output),
                                    status: MessageStatus::Streaming,
                                };
                                if let Err(_) = send_json_buffered(&sender_clone, &tool_resp, &mut token_buffer, false).await {
                                    break;
                                }
                                continue;
                            }

                            if !resp.token.is_empty() {
                                let (text, tool) = parser.push(&resp.token);
                                if let Some(t) = text {
                                    let resp = InferenceResponse {
                                        token: t,
                                        status: MessageStatus::Streaming,
                                    };
                                    if let Err(_) = send_json_buffered(&sender_clone, &resp, &mut token_buffer, false).await {
                                        break;
                                    }
                                }
                                if let Some(tc) = tool {
                                    let tool_def = {
                                         let registry = state_clone.tool_registry.read().unwrap();
                                         registry.list_tools().into_iter().find(|t| t.name == tc.function.name).cloned()
                                    };

                                    println!("GRPC_TOOL_EXECUTION: Executing tool '{}' via ToolExecutor (text-based)", tc.function.name);
                                    let output = if let Some(td) = tool_def {
                                        let mut executor = state_clone.tool_executor.write().await;
                                        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                                        match executor.execute_tool(&td, &args).await {
                                            Ok(res) => res,
                                            Err(e) => format!("Error executing tool: {}", e),
                                        }
                                    } else {
                                         format!("Error: Tool '{}' not found", tc.function.name)
                                    };

                                    let tool_resp = InferenceResponse {
                                        token: output,
                                        status: MessageStatus::ToolCall(ToolCallInfo {
                                            function_name: tc.function.name.clone(),
                                            arguments_json: tc.function.arguments.clone(),
                                        }),
                                    };
                                    if let Err(_) = send_json(&sender_clone, &tool_resp).await {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ =
                            send_error(&sender_clone, &format!("Inference failed: {}", e)).await;
                    }
                }
                
                // Final buffer flush to ensure no tokens are left behind
                if !token_buffer.is_empty() {
                    let final_flush = InferenceResponse {
                        token: token_buffer.flush(),
                        status: MessageStatus::Streaming,
                    };
                    let _ = send_json(&sender_clone, &final_flush).await;
                }
            });
        }
    }
    println!("Websocket disconnected");
}

fn map_status(resp: &GrpcInferenceResponse) -> MessageStatus {
    match resp.status {
        0 => MessageStatus::Streaming,
        1 => MessageStatus::Success,
        2 => MessageStatus::Error(resp.error_message.clone()),
        3 => {
            if let Some(tc) = &resp.tool_call {
                MessageStatus::ToolCall(ToolCallInfo {
                    function_name: tc.function_name.clone(),
                    arguments_json: tc.arguments_json.clone(),
                })
            } else {
                MessageStatus::Error("ToolCall missing data".into())
            }
        }
        _ => MessageStatus::Error("Unknown status".to_string()),
    }
}

async fn send_json(
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    resp: &InferenceResponse,
) -> Result<(), axum::Error> {
    if let Ok(json) = serde_json::to_string(resp) {
        let mut s = sender.lock().await;
        s.send(Message::Text(json)).await.map_err(|e| {
            eprintln!("WS send error: {}", e);
            axum::Error::new(e)
        })?;
    }
    Ok(())
}

// Buffered version for smooth streaming
async fn send_json_buffered(
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    resp: &InferenceResponse,
    buffer: &mut TokenBuffer,
    bypass_buffer: bool,
) -> Result<(), axum::Error> {
    match resp.status {
        MessageStatus::ToolCall(_) | MessageStatus::Success | MessageStatus::Error(_) => {
            // Flush buffer and send immediately for tool calls, success, and errors
            if !buffer.is_empty() {
                let flush_resp = InferenceResponse {
                    token: buffer.flush(),
                    status: MessageStatus::Streaming,
                };
                if let Err(e) = send_json(sender, &flush_resp).await {
                    eprintln!("Error flushing buffer during immediate send: {}", e);
                    // Try to send the immediate message anyway
                    return send_json(sender, resp).await;
                }
            }
            send_json(sender, resp).await?;
        }
        MessageStatus::Queued => {
            // Queue status - treat as immediate since it's control flow
            send_json(sender, resp).await?;
        }
        MessageStatus::Streaming => {
            if bypass_buffer {
                // Send immediately if bypassing buffer
                send_json(sender, resp).await?;
            } else {
                // Add to buffer
                buffer.add_token(&resp.token);
                
                // Check if we should flush
                if buffer.should_flush(false) {
                    let flush_resp = InferenceResponse {
                        token: buffer.flush(),
                        status: MessageStatus::Streaming,
                    };
                    if let Err(e) = send_json(sender, &flush_resp).await {
                        eprintln!("Error sending buffered tokens: {}", e);
                        // Don't return error - just log and continue
                        // Buffer will accumulate and try again on next flush
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_error(
    sender: &Arc<Mutex<futures::stream::SplitSink<WebSocket, Message>>>,
    msg: &str,
) -> Result<(), axum::Error> {
    let resp = InferenceResponse {
        token: "".into(),
        status: MessageStatus::Error(msg.to_string()),
    };
    send_json(sender, &resp).await
}

async fn trigger_inference(
    State(state): State<AppState>,
    Json(payload): Json<InferenceRequest>,
) -> Json<String> {
    let model_name = if let Some(agent_id) = &payload.agent_id {
        let agents = state.agents.read().unwrap();
        agents
            .get(agent_id)
            .map(|a| a.model.clone())
            .unwrap_or_default()
    } else {
        "".to_string()
    };

    let node_address = {
        let nodes = state.nodes.read().unwrap();
        if nodes.is_empty() {
            return Json("Error: No nodes registered".to_string());
        }
        let mut next_node_idx = state.next_node.write().unwrap();
        let node_vec: Vec<_> = nodes.values().collect();
        let node = node_vec[*next_node_idx % node_vec.len()];
        *next_node_idx = (*next_node_idx + 1) % node_vec.len();
        node.address.clone()
    };

    let mut client = match InferenceServiceClient::connect(node_address).await {
        Ok(client) => client,
        Err(e) => return Json(format!("Failed to connect to node: {}", e)),
    };

    // Get tools to include in request
    let tools = {
        let registry = state.tool_registry.read().unwrap();
        registry.list_tools().into_iter().map(convert_common_tool_to_protobuf).collect::<Vec<_>>()
    };

 let request = tonic::Request::new(GrpcInferenceRequest {
        prompt: payload.prompt,
        max_tokens: payload.max_tokens as u32,
        temperature: payload.temperature,
        image_data: vec![],
        stop: vec![],
        model_name,
        tools: tools,
    });

    let mut stream = match client.run_inference(request).await {
        Ok(response) => response.into_inner(),
        Err(e) => return Json(format!("Inference failed: {}", e)),
    };

    let mut tokens = Vec::new();
    while let Some(feature) = stream.next().await {
        if let Ok(resp) = feature {
            tokens.push(resp.token);
        }
    }

    Json(format!("Inference complete. Tokens: {:?}", tokens))
}

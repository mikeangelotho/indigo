use std::collections::HashMap;
use std::time::Duration;
use anyhow::{Result, anyhow, Context};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;
use serde_json::Value;
use reqwest::Client;
use glob::glob;
use regex::Regex;

use indigo_common::{ToolDefinition, ToolConfig, WasmConfig, HttpConfig, McpConfig, CliConfig, NativeConfig};

pub struct ToolExecutor {
    http_client: Client,
    mcp_servers: HashMap<String, McpServerHandle>,
}

#[derive(Clone)]
pub struct McpServerHandle {
    pub server_type: String,
    pub config: McpConfig,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
            mcp_servers: HashMap::new(),
        }
    }

    pub async fn execute_tool(&mut self, tool: &ToolDefinition, arguments: &Value) -> Result<String> {
        match &tool.config {
            ToolConfig::Cli(config) => self.execute_cli_tool(config, arguments).await,
            ToolConfig::Http(config) => self.execute_http_tool(config, arguments).await,
            ToolConfig::Wasm(config) => self.execute_wasm_tool(config, arguments).await,
            ToolConfig::Mcp(config) => self.execute_mcp_tool(config, &tool.name, arguments).await,
            ToolConfig::Native(config) => self.execute_native_tool(config, arguments).await,
        }
    }

    async fn execute_native_tool(&self, config: &NativeConfig, arguments: &Value) -> Result<String> {
        match config.name.as_str() {
            "bash" | "run_shell" => {
                 let cmd = arguments.get("command").or_else(|| arguments.get("cmd")).or_else(|| arguments.get("input")).and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'command' argument"))?;
                 
                 #[cfg(target_os = "windows")]
                 let output = TokioCommand::new("cmd").arg("/C").arg(cmd).output().await
                     .map_err(|e| anyhow!("Failed to execute process: {}", e))?;
                 #[cfg(not(target_os = "windows"))]
                 let output = TokioCommand::new("bash").arg("-c").arg(cmd).output().await
                     .map_err(|e| anyhow!("Failed to execute process: {}", e))?;

                 let stdout = String::from_utf8_lossy(&output.stdout);
                 let stderr = String::from_utf8_lossy(&output.stderr);
                 Ok(format!("Output:\n{}\nErrors:\n{}", stdout, stderr))
            }
            "read" | "read_file" => {
                let path = arguments.get("path").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
                std::fs::read_to_string(path).map_err(|e| anyhow!("Failed to read file: {}", e))
            }
            "write_file" => {
                let path = arguments.get("path").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
                let content = arguments.get("content").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'content' argument"))?;
                std::fs::write(path, content).map_err(|e| anyhow!("Failed to write file: {}", e))?;
                Ok(format!("Successfully wrote to {}", path))
            }
            "glob" | "list_files" => {
                let pattern = arguments.get("pattern").or_else(|| arguments.get("path")).and_then(|v| v.as_str()).unwrap_or("*");
                let paths = glob(pattern).map_err(|e| anyhow!("Invalid glob pattern: {}", e))?;
                let names: Vec<String> = paths.filter_map(|entry| entry.ok().map(|p| p.display().to_string())).collect();
                Ok(if names.is_empty() { "No files found".to_string() } else { names.join("\n") })
            }
            "grep" => {
                let pattern_str = arguments.get("pattern").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing 'pattern' argument"))?;
                let include = arguments.get("include").or_else(|| arguments.get("path")).and_then(|v| v.as_str()).unwrap_or("*");
                let re = Regex::new(pattern_str).map_err(|e| anyhow!("Invalid regex: {}", e))?;
                
                let paths = glob(include).map_err(|e| anyhow!("Invalid glob pattern: {}", e))?;
                let mut matches = Vec::new();
                for entry in paths.filter_map(Result::ok) {
                    if entry.is_file() {
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            for (i, line) in content.lines().enumerate() {
                                if re.is_match(line) {
                                    matches.push(format!("{}:{}: {}", entry.display(), i + 1, line));
                                }
                            }
                        }
                    }
                }
                Ok(if matches.is_empty() { "No matches found".to_string() } else { matches.join("\n") })
            }
            _ => Err(anyhow!("Unknown native tool: {}", config.name)),
        }
    }

    async fn execute_cli_tool(&self, config: &CliConfig, arguments: &Value) -> Result<String> {
        let mut cmd = TokioCommand::new(&config.command);
        
        // Add configured args
        for arg in &config.args {
            cmd.arg(arg);
        }
        
        // Add arguments from tool call
        if let Some(args_obj) = arguments.as_object() {
            for (key, value) in args_obj {
                match value {
                    Value::String(s) => cmd.arg(format!("--{}={}", key, s)),
                    Value::Bool(b) => cmd.arg(format!("--{}={}", key, b)),
                    Value::Number(n) => cmd.arg(format!("--{}={}", key, n)),
                    _ => cmd.arg(format!("--{}={}", key, value)),
                };
            }
        }
        
        // Set working directory
        if !config.working_dir.is_empty() && config.working_dir != "." {
            cmd.current_dir(&config.working_dir);
        }
        
        // Set environment variables
        for (key, val) in &config.environment {
            cmd.env(key, val);
        }
        
        // Execute with timeout
        let duration = Duration::from_secs(config.timeout as u64);
        let output = timeout(duration, cmd.output()).await
            .map_err(|_| anyhow!("Command execution timed out"))?
            .map_err(|e| anyhow!("Failed to execute command: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Command failed: {}", stderr));
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    }

    async fn execute_http_tool(&self, config: &HttpConfig, arguments: &Value) -> Result<String> {
        let mut request = match config.method.to_uppercase().as_str() {
            "GET" => self.http_client.get(&config.endpoint),
            "POST" => self.http_client.post(&config.endpoint),
            "PUT" => self.http_client.put(&config.endpoint),
            "DELETE" => self.http_client.delete(&config.endpoint),
            "PATCH" => self.http_client.patch(&config.endpoint),
            _ => return Err(anyhow!("Unsupported HTTP method: {}", config.method)),
        };

        // Set headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set authentication
        if !config.auth_token.is_empty() {
            match config.auth_type.as_str() {
                "Bearer" | "bearer" => request = request.bearer_auth(&config.auth_token),
                "Basic" | "basic" => request = request.basic_auth("", Some(&config.auth_token)),
                _ => request = request.header("Authorization", &config.auth_token),
            }
        }

        // Add arguments as JSON body for POST/PUT/PATCH requests
        if ["POST", "PUT", "PATCH"].contains(&config.method.to_uppercase().as_str()) {
            request = request.json(arguments);
        } else if ["GET", "DELETE"].contains(&config.method.to_uppercase().as_str()) {
            // Add query parameters for GET/DELETE
            if let Some(obj) = arguments.as_object() {
                let mut query_params = Vec::new();
                for (key, value) in obj {
                    query_params.push(format!("{}={}", key, value));
                }
                if !query_params.is_empty() {
                    let query_string = query_params.join("&");
                    let url_with_query = if config.endpoint.contains('?') {
                        format!("{}&{}", config.endpoint, query_string)
                    } else {
                        format!("{}?{}", config.endpoint, query_string)
                    };
                    request = match config.method.to_uppercase().as_str() {
                        "GET" => self.http_client.get(&url_with_query),
                        "DELETE" => self.http_client.delete(&url_with_query),
                        _ => request,
                    };
                }
            }
        }

        // Note: reqwest doesn't have danger_accept_invalid_certs in the public API
        // SSL verification can be disabled via danger_accept_invalid_certs in reqwest::ClientBuilder
        
        let response = request.send().await
            .map_err(|e| anyhow!("Failed to send HTTP request: {}", e))?;
        
        if !response.status().is_success() {
            return Err(anyhow!("HTTP request failed with status: {}", response.status()));
        }

        let text = response.text().await.with_context(|| "Failed to read response body")?;
        Ok(text)
    }

    async fn execute_wasm_tool(&self, _config: &WasmConfig, _arguments: &Value) -> Result<String> {
        #[cfg(feature = "wasm")]
        {
            use wasmtime::*;
            use std::fs;
            
            // Check if WASM module exists
            if !fs::metadata(&config.module_path).is_ok() {
                return Err(anyhow!("WASM module not found: {}", config.module_path));
            }
            
            let mut engine_config = Config::new();
            engine_config.consume_fuel(true);
            engine_config.wasm_component_model(false);
            
            let engine = Engine::new(&engine_config)?;
            let module = Module::from_file(&engine, &config.module_path)?;
            let mut store = Store::new(&engine, WasmState {
                memory_limit: config.memory_limit,
                memory_used: 0,
                result_buffer: String::new(),
            });
            
            // Set fuel limits for timeout control
            store.add_fuel(config.timeout * 1_000_000)?; // Convert to approximate fuel units
            
            // Create WASI context for filesystem access
            let wasi = WasiCtxBuilder::new()
                .inherit_stdio()
                .build();
            store.data_mut().wasi = Some(wasi);
            
            // Import function that writes to result buffer
            let result_buf = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let result_buf_clone = result_buf.clone();
            
            let host_func = Func::wrap(&mut store, move |mut caller: Caller<'_, WasmState>, ptr: i32, len: i32| -> Result<(), Trap> {
                let mem = match caller.get_export("memory") {
                    Some(Extern::Memory(mem)) => mem,
                    _ => return Err(Trap::new("failed to find host memory")),
                };
                
                let data = mem
                    .data(&caller)
                    .get(ptr as usize..)
                    .and_then(|arr| arr.get(..len as usize))
                    .ok_or_else(|| Trap::new("out of bounds memory access"))?;
                
                let string = String::from_utf8_lossy(data);
                if let Ok(mut buf) = result_buf_clone.lock() {
                    buf.push_str(&string);
                }
                Ok(())
            });
            
            let instance = Instance::new(&mut store, &module, &[host_func.into()])?;
            
            // Prepare arguments - serialize to JSON and pass as string
            let args_json = serde_json::to_string(arguments)?;
            let args_bytes = args_json.as_bytes();
            
            // Allocate memory for arguments in WASM module
            let memory = instance
                .get_typed_func::<i32, i32>(&mut store, "allocate")?
                .or_else(|_| {
                    // Fallback: try to find memory export and allocate manually
                    Ok(1024) // Fixed allocation offset
                })?;
            
            let args_ptr = if let Ok(alloc_func) = instance.get_typed_func::<i32, i32>(&mut store, "allocate") {
                alloc_func.call(&mut store, args_bytes.len() as i32)?
            } else {
                // Simple allocation at offset 1024 if no allocate function
                1024
            };
            
            // Write arguments to WASM memory
            let memory = instance
                .get_export(&mut store, "memory")
                .and_then(|e| e.into_memory())
                .ok_or_else(|| anyhow!("WASM module must export memory"))?;
            
            let memory_data = memory.data_mut(&mut store);
            let start = args_ptr as usize;
            if start + args_bytes.len() > memory_data.len() {
                return Err(anyhow!("Insufficient WASM memory for arguments"));
            }
            memory_data[start..start + args_bytes.len()].copy_from_slice(args_bytes);
            
            // Call the main function
            let func = instance
                .get_typed_func::<(i32, i32), i32>(&mut store, &config.function_name)
                .or_else(|_| {
                    // Try alternative function signatures
                    instance.get_typed_func::<i32, i32>(&mut store, &config.function_name)
                })?;
            
            let result = func.call(&mut store, (args_ptr, args_bytes.len() as i32))?;
            
            // Get the result from the buffer
            let output = if let Ok(buf) = result_buf.lock() {
                if buf.is_empty() {
                    // If no host output, try to read from WASM memory
                    let output_ptr = result;
                    if output_ptr > 0 {
                        // Find null terminator or read fixed size
                        let output_data = memory.data(&store)
                            .get(output_ptr as usize..)
                            .and_then(|data| {
                                data.iter()
                                    .take_while(|&&b| b != 0)
                                    .cloned()
                                    .collect::<Vec<_>>()
                            });
                        if let Some(bytes) = output_data {
                            String::from_utf8_lossy(&bytes).to_string()
                        } else {
                            format!("WASM function returned: {}", result)
                        }
                    } else {
                        format!("WASM function executed with result: {}", result)
                    }
                } else {
                    buf.clone()
                }
            } else {
                format!("WASM function executed with result: {}", result)
            };
            
            Ok(output)
        }
        
        #[cfg(not(feature = "wasm"))]
        {
            Err(anyhow!("WASM execution not supported - compile with 'wasm' feature"))
        }
    }

    async fn execute_mcp_tool(&mut self, config: &McpConfig, tool_name: &str, arguments: &Value) -> Result<String> {
        // Check if we already have this MCP server connected
        if !self.mcp_servers.contains_key(&config.server_name) {
            self.connect_mcp_server(config).await?;
        }

        let server_handle = self.mcp_servers.get(&config.server_name)
            .ok_or_else(|| anyhow!("MCP server '{}' not connected", config.server_name))?;
        
        match server_handle.server_type.as_str() {
            "stdio" => self.execute_stdio_mcp_tool(server_handle, tool_name, arguments).await,
            "http" => self.execute_http_mcp_tool(server_handle, tool_name, arguments).await,
            _ => Err(anyhow!("Unsupported MCP server type: {}", server_handle.server_type)),
        }
    }

    async fn connect_mcp_server(&mut self, config: &McpConfig) -> Result<()> {
        let handle = McpServerHandle {
            server_type: config.server_type.clone(),
            config: config.clone(),
        };

        // Validate MCP server configuration
        if config.server_type == "stdio" {
            if config.args.is_empty() {
                return Err(anyhow!("stdio MCP server requires at least a command in args"));
            }
        } else if config.server_type == "http" {
            if config.endpoint.is_empty() {
                return Err(anyhow!("HTTP MCP server requires an endpoint"));
            }
            // Test HTTP connection
            let test_url = format!("{}/health", config.endpoint.trim_end_matches('/'));
            let response = self.http_client.get(&test_url).send().await;
            if let Err(e) = response {
                return Err(anyhow!("Failed to connect to MCP HTTP server at {}: {}", config.endpoint, e));
            }
        }
        
        self.mcp_servers.insert(config.server_name.clone(), handle);
        Ok(())
    }

    async fn execute_stdio_mcp_tool(&self, handle: &McpServerHandle, tool_name: &str, arguments: &Value) -> Result<String> {
        use tokio::process::Command;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        // Build the command from MCP server configuration
        let mut cmd = Command::new(&handle.config.args[0]);
        if handle.config.args.len() > 1 {
            cmd.args(&handle.config.args[1..]);
        }
        
        // Set environment variables
        for (key, val) in &handle.config.environment {
            cmd.env(key, val);
        }
        
        // Spawn the MCP server process
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn MCP server: {}", e))?;
            
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to get stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to get stdout"))?;
        
        // Create MCP tool call request
        let tool_call = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });
        
        // Send the request
        let request = serde_json::to_string(&tool_call)?;
        let mut stdin = stdin;
        stdin.write_all(request.as_bytes()).await
            .map_err(|e| anyhow!("Failed to write to MCP server: {}", e))?;
        stdin.write_all(b"\n").await
            .map_err(|e| anyhow!("Failed to write newline to MCP server: {}", e))?;
        stdin.flush().await
            .map_err(|e| anyhow!("Failed to flush MCP server stdin: {}", e))?;
        
        // Read the response
        let mut stdout = stdout;
        let mut response_buf = Vec::new();
        let mut temp_buf = [0; 1024];
        
        // Read until we get a complete JSON response or timeout
        let timeout_duration = std::time::Duration::from_secs(30);
        let read_future = async {
            loop {
                match stdout.read(&mut temp_buf).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        response_buf.extend_from_slice(&temp_buf[..n]);
                        // Try to parse the response
                        if let Ok(response_str) = std::str::from_utf8(&response_buf) {
                            if let Ok(response) = serde_json::from_str::<serde_json::Value>(response_str) {
                                if response.get("result").is_some() || response.get("error").is_some() {
                                    return Some(response);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from MCP server: {}", e);
                        break;
                    }
                }
            }
            None
        };
        
        let response_opt = tokio::time::timeout(timeout_duration, read_future).await
            .map_err(|_| anyhow!("MCP server communication timeout"))?;
            
        let response = response_opt.ok_or_else(|| anyhow!("No valid response from MCP server"))?;
        
        // Kill the child process
        let _ = child.kill().await;
        
        // Extract the result
        if let Some(result) = response.get("result") {
            if let Some(content) = result.get("content") {
                if let Some(text) = content.as_str() {
                    return Ok(text.to_string());
                } else if let Some(arr) = content.as_array() {
                    // Handle multiple content items
                    let texts: Vec<String> = arr.iter()
                        .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                        .collect();
                    if !texts.is_empty() {
                        return Ok(texts.join("\n"));
                    }
                }
            }
            Ok(serde_json::to_string_pretty(result)?)
        } else if let Some(error) = response.get("error") {
            Err(anyhow!("MCP tool error: {}", serde_json::to_string_pretty(error)?))
        } else {
            Err(anyhow!("Invalid MCP response format"))
        }
    }

    async fn execute_http_mcp_tool(&self, handle: &McpServerHandle, tool_name: &str, arguments: &Value) -> Result<String> {
        let endpoint = format!("{}/tools/{}", handle.config.endpoint, tool_name);
        
        let response = self.http_client
            .post(&endpoint)
            .json(arguments)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send MCP HTTP request: {}", e))?;
            
        if !response.status().is_success() {
            return Err(anyhow!("MCP HTTP request failed with status: {}", response.status()));
        }

        let text = response.text().await.with_context(|| "Failed to read MCP response body")?;
        Ok(text)
    }

    pub async fn register_mcp_server(&mut self, config: &McpConfig) -> Result<()> {
        self.connect_mcp_server(config).await
    }

    pub fn list_mcp_servers(&self) -> Vec<String> {
        self.mcp_servers.keys().cloned().collect()
    }

    pub async fn execute_mcp_tool_direct(&mut self, server_name: &str, tool_name: &str, arguments_json: &str) -> Result<String> {
        let arguments: serde_json::Value = serde_json::from_str(arguments_json)?;
        
        // Find the MCP server configuration and clone it to avoid borrow issues
        let server_config = {
            let server_handle = self.mcp_servers.get(server_name)
                .ok_or_else(|| anyhow!("MCP server not found: {}", server_name))?;
            server_handle.config.clone()
        };
        
        self.execute_mcp_tool(&server_config, tool_name, &arguments).await
    }
}

#[cfg(feature = "wasm")]
struct WasmState {
    memory_limit: u32,
    memory_used: u32,
    result_buffer: String,
    wasi: Option<wasmtime_wasi::WasiCtx>,
}

#[cfg(feature = "wasm")]
struct WasmLimiter {
    memory_limit: u32,
    current_memory: u32,
}

#[cfg(feature = "wasm")]
impl ResourceLimiter for WasmLimiter {
    fn memory_growing(&mut self, current: usize, desired: usize, maximum: Option<usize>) -> bool {
        let desired_mb = (desired / (1024 * 1024)) as u32;
        if desired_mb <= self.memory_limit {
            self.current_memory = desired_mb;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_cli_tool_execution() {
        let executor = ToolExecutor::new();
        
        let config = CliConfig {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            working_dir: ".".to_string(),
            environment: HashMap::new(),
            timeout: 5,
        };

        let result = executor.execute_cli_tool(&config, &serde_json::json!({})).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn test_http_tool_execution() {
        let executor = ToolExecutor::new();
        
        let config = HttpConfig {
            endpoint: "https://httpbin.org/get".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            auth_type: "".to_string(),
            auth_token: "".to_string(),
            verify_ssl: true,
        };

        let result = executor.execute_http_tool(&config, &serde_json::json!({})).await;
        assert!(result.is_ok());
    }
}
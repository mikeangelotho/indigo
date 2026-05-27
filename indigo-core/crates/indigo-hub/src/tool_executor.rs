//! Tool executor — dispatches to native plugins or forwards to remote agents.
//!
//! The executor holds a `NativeToolRegistry` for local tools and a list of connected
//! agents for remote tool delegation. Maintains backward compatibility with the
//! existing `execute_tool(&ToolDefinition, &Value)` API.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tool_plugin::NativeToolRegistry;
use indigo_common::{McpConfig, ToolConfig, ToolDefinition};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub name: String,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Unified tool executor that delegates to plugins or remote agents.
pub struct ToolExecutor {
    /// Registry of native tool plugins (local execution).
    pub registry: NativeToolRegistry,
    /// Registered MCP server configurations.
    pub mcp_servers: Vec<McpConfig>,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            registry: NativeToolRegistry::new(),
            mcp_servers: Vec::new(),
        }
    }

    /// Execute a tool call by name and arguments (new plugin-based API).
    pub fn execute_tool_call(&self, name: &str, args: &Value) -> Result<String> {
        if let Some(plugin) = self.registry.get(name) {
            plugin.execute(args)
        } else {
            Err(anyhow!(
                "No tool handler found for '{}'. Available native tools: {}",
                name,
                self.registry.list_names().join(", ")
            ))
        }
    }

    /// Execute a single tool call using the legacy ToolDefinition API.
    ///
    /// This maintains backward compatibility with existing code that passes
    /// a ToolDefinition and arguments. It extracts the tool name from the
    /// definition's config and dispatches to the plugin registry.
    pub async fn execute_tool(
        &self,
        tool_def: &ToolDefinition,
        args: &Value,
    ) -> Result<String> {
        let tool_name = match &tool_def.config {
            ToolConfig::Native(cfg) => cfg.name.clone(),
            _ => tool_def.name.clone(),
        };

        self.execute_tool_call(&tool_name, args)
    }

    /// Execute multiple tool calls.
    pub async fn execute_tools(
        &self,
        tool_calls: &[ToolCall],
    ) -> Vec<ToolResult> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for tc in tool_calls {
            match self.execute_tool_call(&tc.name, &tc.arguments) {
                Ok(result) => results.push(ToolResult {
                    name: tc.name.clone(),
                    result,
                    error: None,
                }),
                Err(e) => results.push(ToolResult {
                    name: tc.name.clone(),
                    result: String::new(),
                    error: Some(e.to_string()),
                }),
            }
        }

        results
    }

    /// Execute an MCP tool directly (legacy stub — delegates to plugin registry
    /// if an MCP plugin is registered, otherwise returns an error).
    pub async fn execute_mcp_tool_direct(
        &self,
        _server_name: &str,
        tool_name: &str,
        arguments_json: &str,
    ) -> Result<String> {
        let args: Value = serde_json::from_str(arguments_json)
            .map_err(|e| anyhow!("Invalid JSON arguments: {}", e))?;
        self.execute_tool_call(tool_name, &args)
    }

    /// Get OpenAI-compatible tool definitions for all registered native tools.
    pub fn get_tool_definitions(&self) -> Vec<Value> {
        let mut tools = Vec::new();
        for name in self.registry.list_names() {
            if let Some(p) = self.registry.get(name) {
                tools.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": p.name(),
                        "description": p.description(),
                        "parameters": p.parameters()
                    }
                }));
            }
        }
        tools
    }

    /// List all registered MCP server names.
    pub fn list_mcp_servers(&self) -> Vec<String> {
        self.mcp_servers.iter().map(|s| s.server_name.clone()).collect()
    }

    /// Register a new MCP server configuration.
    pub async fn register_mcp_server(&mut self, config: &McpConfig) -> Result<()> {
        // Check for duplicate
        if self.mcp_servers.iter().any(|s| s.server_name == config.server_name) {
            return Err(anyhow!("MCP server '{}' is already registered", config.server_name));
        }
        self.mcp_servers.push(config.clone());
        Ok(())
    }
}

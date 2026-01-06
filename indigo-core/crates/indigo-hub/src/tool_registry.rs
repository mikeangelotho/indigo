use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use indigo_common::{
    CliConfig, ToolConfig, ToolDefinition, ToolType,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };

        // Initialize with built-in tools
        registry.register_builtin_tools();
        registry
    }

    fn register_builtin_tools(&mut self) {
        let now = Utc::now().to_rfc3339();

        // list_files tool
        let list_files = ToolDefinition {
            id: "builtin_list_files".to_string(),
            name: "list_files".to_string(),
            description: "List files and directories in a specified path".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory path to list (default: current directory)"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "List files recursively (default: false)",
                        "default": false
                    }
                }
            }),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "ls".to_string(),
                args: vec!["-la".to_string()],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec!["file:read".to_string()],
            node_compatible: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // read_file tool
        let read_file = ToolDefinition {
            id: "builtin_read_file".to_string(),
            name: "read_file".to_string(),
            description: "Read contents of a file".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start reading from (optional)",
                        "minimum": 0
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read (optional)",
                        "minimum": 1
                    }
                },
                "required": ["path"]
            }),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "cat".to_string(),
                args: vec![],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec!["file:read".to_string()],
            node_compatible: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // write_file tool
        let write_file = ToolDefinition {
            id: "builtin_write_file".to_string(),
            name: "write_file".to_string(),
            description: "Write content to a file (creates or overwrites)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "tee".to_string(),
                args: vec![],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec!["file:write".to_string()],
            node_compatible: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        // run_shell tool
        let run_shell = ToolDefinition {
            id: "builtin_run_shell".to_string(),
            name: "run_shell".to_string(),
            description: "Execute shell commands".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Working directory for command execution (optional)"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Command timeout in seconds (default: 30)",
                        "default": 30,
                        "minimum": 1,
                        "maximum": 300
                    }
                },
                "required": ["command"]
            }),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "sh".to_string(),
                args: vec!["-c".to_string()],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec!["shell:execute".to_string()],
            node_compatible: true,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        self.tools.insert(list_files.id.clone(), list_files);
        self.tools.insert(read_file.id.clone(), read_file);
        self.tools.insert(write_file.id.clone(), write_file);
        self.tools.insert(run_shell.id.clone(), run_shell);
    }

    pub fn register_tool(&mut self, tool: ToolDefinition) -> Result<()> {
        self.validate_tool(&tool)?;
        self.tools.insert(tool.id.clone(), tool);
        Ok(())
    }

    pub fn unregister_tool(&mut self, tool_id: &str) -> Result<()> {
        if self.tools.remove(tool_id).is_none() {
            return Err(anyhow!("Tool not found: {}", tool_id));
        }
        Ok(())
    }

    pub fn update_tool(&mut self, tool_id: &str, tool: ToolDefinition) -> Result<()> {
        if !self.tools.contains_key(tool_id) {
            return Err(anyhow!("Tool not found: {}", tool_id));
        }
        self.validate_tool(&tool)?;
        self.tools.insert(tool_id.to_string(), tool);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_tool(&self, tool_id: &str) -> Option<&ToolDefinition> {
        self.tools.get(tool_id)
    }

    pub fn list_tools(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    pub fn list_tools_by_type(&self, tool_type: &ToolType) -> Vec<&ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| &tool.tool_type == tool_type)
            .collect()
    }

    #[allow(dead_code)]
    pub fn list_node_compatible_tools(&self) -> Vec<&ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| tool.node_compatible)
            .collect()
    }

    pub fn validate_tool(&self, tool: &ToolDefinition) -> Result<()> {
        if tool.name.is_empty() {
            return Err(anyhow!("Tool name cannot be empty"));
        }

        if tool.id.is_empty() {
            return Err(anyhow!("Tool ID cannot be empty"));
        }

        // Validate JSON schema
        if let Err(e) = serde_json::from_value::<serde_json::Value>(tool.parameters.clone()) {
            return Err(anyhow!("Invalid parameters schema: {}", e));
        }

        // Validate tool-specific configuration
        match &tool.config {
            ToolConfig::Wasm(config) => {
                if config.module_path.is_empty() {
                    return Err(anyhow!("WASM module path cannot be empty"));
                }
                if config.function_name.is_empty() {
                    return Err(anyhow!("WASM function name cannot be empty"));
                }
            }
            ToolConfig::Http(config) => {
                if config.endpoint.is_empty() {
                    return Err(anyhow!("HTTP endpoint cannot be empty"));
                }
                if !config.endpoint.starts_with("http://")
                    && !config.endpoint.starts_with("https://")
                {
                    return Err(anyhow!("HTTP endpoint must start with http:// or https://"));
                }
            }
            ToolConfig::Mcp(config) => {
                if config.server_name.is_empty() {
                    return Err(anyhow!("MCP server name cannot be empty"));
                }
                if config.server_type != "stdio" && config.server_type != "http" {
                    return Err(anyhow!("MCP server type must be 'stdio' or 'http'"));
                }
            }
            ToolConfig::Cli(config) => {
                if config.command.is_empty() {
                    return Err(anyhow!("CLI command cannot be empty"));
                }
            }
            ToolConfig::Native(config) => {
                if config.name.is_empty() {
                    return Err(anyhow!("Native tool name cannot be empty"));
                }
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn export_tools(&self) -> Vec<ToolDefinition> {
        self.tools.values().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn import_tools(&mut self, tools: Vec<ToolDefinition>) -> Result<()> {
        for tool in tools {
            self.register_tool(tool)?;
        }
        Ok(())
    }
}

pub type SharedToolRegistry = Arc<RwLock<ToolRegistry>>;

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tool() {
        let mut registry = ToolRegistry::new();

        let tool = ToolDefinition {
            id: "test_tool".to_string(),
            name: "test".to_string(),
            description: "Test tool".to_string(),
            parameters: serde_json::json!({}),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "echo".to_string(),
                args: vec![],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec![],
            node_compatible: true,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        assert!(registry.register_tool(tool.clone()).is_ok());
        assert_eq!(registry.get_tool("test_tool").unwrap().name, "test");
    }

    #[test]
    fn test_invalid_tool() {
        let mut registry = ToolRegistry::new();

        let invalid_tool = ToolDefinition {
            id: "".to_string(), // Invalid empty ID
            name: "test".to_string(),
            description: "Test tool".to_string(),
            parameters: serde_json::json!({}),
            tool_type: ToolType::Native,
            config: ToolConfig::Cli(CliConfig {
                command: "echo".to_string(),
                args: vec![],
                working_dir: ".".to_string(),
                environment: HashMap::new(),
                timeout: 30,
            }),
            permissions: vec![],
            node_compatible: true,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        assert!(registry.register_tool(invalid_tool).is_err());
    }
}

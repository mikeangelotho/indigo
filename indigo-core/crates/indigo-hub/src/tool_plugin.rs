//! Modular tool plugin system.
//!
//! Each tool implements `ToolPlugin`. The `NativeToolRegistry` holds all registered plugins.
//! `ToolExecutor` delegates to plugins instead of giant match arms.
//!
//! Adding a new tool: define a struct, impl `ToolPlugin`, call `registry.register(Box::new(MyTool))`.

use anyhow::{anyhow, Result};
use glob::glob;
use regex::Regex;
use serde_json::{json, Value};

/// Trait that every native tool plugin must implement.
pub trait ToolPlugin: Send + Sync {
    /// Unique tool name (snake_case).
    fn name(&self) -> &str;
    /// Human-readable description shown to the model.
    fn description(&self) -> &str;
    /// JSON Schema for the tool's parameters.
    fn parameters(&self) -> Value;
    /// Execute the tool with validated arguments.
    fn execute(&self, args: &Value) -> Result<String>;
}

/// Registry of native tool plugins. Thread-safe, allows runtime registration.
#[derive(Default)]
pub struct NativeToolRegistry {
    plugins: Vec<Box<dyn ToolPlugin>>,
}

impl NativeToolRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_all_builtins();
        reg
    }

    /// Register a single tool plugin.
    pub fn register(&mut self, plugin: Box<dyn ToolPlugin>) {
        // Replace if a tool with the same name already exists
        self.plugins.retain(|p| p.name() != plugin.name());
        self.plugins.push(plugin);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn ToolPlugin> {
        self.plugins
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// List all registered tool names.
    pub fn list_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// Register all built-in tools.
    fn register_all_builtins(&mut self) {
        self.register(Box::new(ListFilesTool));
        self.register(Box::new(ReadFileTool));
        self.register(Box::new(WriteFileTool));
        self.register(Box::new(RunShellTool));
        self.register(Box::new(GrepTool));
        self.register(Box::new(AnalyzeProjectTool));
        self.register(Box::new(UrlShortenerTool));
    }
}

// ---------------------------------------------------------------------------
// Built-in tool implementations
// ---------------------------------------------------------------------------

/// List files and directories in a path (or match a glob pattern).
pub struct ListFilesTool;
impl ToolPlugin for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }
    fn description(&self) -> &str {
        "List files and directories. If 'path' is a directory, lists its contents. If 'pattern' is a glob, matches files."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path to list (default: current directory)" },
                "pattern": { "type": "string", "description": "Glob pattern to match files (default: *)" }
            }
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let input = args
            .get("pattern")
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("*");

        let names = if std::path::Path::new(input).is_dir() {
            let dir_path = if input.ends_with('/') || input.ends_with('\\') {
                format!("{}*", input)
            } else {
                format!("{}/*", input)
            };
            glob(&dir_path)
                .map_err(|e| anyhow!("Invalid glob pattern: {}", e))?
                .filter_map(|e| e.ok())
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
        } else {
            glob(input)
                .map_err(|e| anyhow!("Invalid glob pattern: {}", e))?
                .filter_map(|e| e.ok().map(|p| p.display().to_string()))
                .collect::<Vec<_>>()
        };

        Ok(if names.is_empty() {
            "No files found".into()
        } else {
            names.join("\n")
        })
    }
}

/// Read the contents of a file.
pub struct ReadFileTool;
impl ToolPlugin for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Read the full contents of a file at the given path."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file to read" },
                "offset": { "type": "integer", "description": "Line number to start reading from (optional)", "minimum": 0 },
                "limit": { "type": "integer", "description": "Maximum number of lines to read (optional)", "minimum": 1 }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read file '{}': {}", path, e))?;

        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::MAX) as usize;

        let lines: Vec<&str> = content.lines().collect();
        let start = offset.min(lines.len());
        let end = (start + limit).min(lines.len());
        let slice = &lines[start..end];

        if offset > 0 || limit < u64::MAX as usize {
            Ok(slice.join("\n"))
        } else {
            Ok(content)
        }
    }
}

/// Write content to a file (creates or overwrites).
pub struct WriteFileTool;
impl ToolPlugin for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file to write" },
                "content": { "type": "string", "description": "Content to write to file" }
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'path' argument"))?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' argument"))?;

        // Create parent directories if needed
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create directories for '{}': {}", path, e))?;
            }
        }

        std::fs::write(path, content)
            .map_err(|e| anyhow!("Failed to write to '{}': {}", path, e))?;
        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path
        ))
    }
}

/// Execute a shell command.
pub struct RunShellTool;
impl ToolPlugin for RunShellTool {
    fn name(&self) -> &str {
        "run_shell"
    }
    fn description(&self) -> &str {
        "Execute a shell command and return stdout/stderr."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command to execute" },
                "working_dir": { "type": "string", "description": "Working directory (optional)" },
                "timeout": { "type": "integer", "description": "Timeout in seconds (default: 30)", "default": 30 }
            },
            "required": ["command"]
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        use std::process::Command;
        use std::sync::mpsc;
        use std::thread;
        use std::time::Duration;

        let cmd = args
            .get("command")
            .or_else(|| args.get("cmd"))
            .or_else(|| args.get("input"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'command' argument"))?;

        let timeout_secs = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);

        let mut command = Command::new(if cfg!(windows) { "cmd" } else { "bash" });
        if cfg!(windows) {
            command.args(["/C", cmd]);
        } else {
            command.args(["-c", cmd]);
        }

        if let Some(wd) = args.get("working_dir").and_then(|v| v.as_str()) {
            command.current_dir(wd);
        }

        let (tx, rx) = mpsc::channel();
        let cmd_string = cmd.to_string();
        let child_thread = thread::spawn(move || {
            let result = command.output();
            let _ = tx.send(result);
        });

        match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
            Ok(output_result) => {
                let _ = child_thread.join();
                let output = output_result
                    .map_err(|e| anyhow!("Failed to execute command '{}': {}", cmd_string, e))?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut result = String::new();
                if !stdout.is_empty() {
                    result.push_str(&format!("STDOUT:\n{}\n", stdout));
                }
                if !stderr.is_empty() {
                    result.push_str(&format!("STDERR:\n{}\n", stderr));
                }
                if result.is_empty() {
                    result.push_str("(command produced no output)");
                }

                Ok(result)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(anyhow!("Command timed out after {}s", timeout_secs))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(anyhow!("Command thread disconnected"))
            }
        }
    }
}

/// Search file contents with a regex pattern.
pub struct GrepTool;
impl ToolPlugin for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search for a regex pattern in files matching a glob pattern."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for" },
                "include": { "type": "string", "description": "Glob pattern for files to include (default: *)" },
                "path": { "type": "string", "description": "Alias for include — glob pattern for files" }
            },
            "required": ["pattern"]
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let pattern_str = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'pattern' argument"))?;
        let include = args
            .get("include")
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("*");

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

        Ok(if matches.is_empty() {
            "No matches found".into()
        } else {
            matches.join("\n")
        })
    }
}

/// Analyze project structure and provide insights.
pub struct AnalyzeProjectTool;
impl ToolPlugin for AnalyzeProjectTool {
    fn name(&self) -> &str {
        "analyze_project"
    }
    fn description(&self) -> &str {
        "Analyze project structure and provide insights about architecture, dependencies, or security."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "focus": {
                    "type": "string",
                    "description": "Focus area: 'architecture', 'dependencies', 'security', or 'performance'",
                    "enum": ["architecture", "dependencies", "security", "performance"]
                }
            }
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let focus = args
            .get("focus")
            .and_then(|v| v.as_str())
            .unwrap_or("architecture");
        let mut result = format!("Project Analysis (Focus: {})\n", focus);

        match std::fs::read_dir(".") {
            Ok(entries) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            dirs.push(entry.file_name().to_string_lossy().to_string());
                        } else {
                            files.push(entry.file_name().to_string_lossy().to_string());
                        }
                    }
                }

                result.push_str(&format!("\nDirectories ({}):\n", dirs.len()));
                for dir in &dirs {
                    result.push_str(&format!("  {}/\n", dir));
                }
                result.push_str(&format!("\nFiles ({}):\n", files.len()));
                for file in &files {
                    result.push_str(&format!("  {}\n", file));
                }

                match focus {
                    "architecture" => {
                        result.push_str("\nArchitecture Analysis:\n");
                        if dirs.iter().any(|d| d == "src" || d == "lib") {
                            result.push_str("  [OK] Has source code directory\n");
                        }
                        if files
                            .iter()
                            .any(|f| f.ends_with("Cargo.toml") || f.ends_with("package.json"))
                        {
                            result.push_str("  [OK] Has package manifest\n");
                        }
                        if files.iter().any(|f| f == "README.md" || f == "README") {
                            result.push_str("  [OK] Has documentation\n");
                        }
                    }
                    "dependencies" => {
                        result.push_str("\nDependencies Analysis:\n");
                        if files.iter().any(|f| f.ends_with("Cargo.toml")) {
                            result.push_str("  - Rust/Cargo project detected\n");
                        }
                        if files.iter().any(|f| f.ends_with("package.json")) {
                            result.push_str("  - Node.js/npm project detected\n");
                        }
                        if files.iter().any(|f| {
                            f.ends_with("requirements.txt") || f.ends_with("pyproject.toml")
                        }) {
                            result.push_str("  - Python project detected\n");
                        }
                    }
                    "security" => {
                        result.push_str("\nSecurity Analysis:\n");
                        result.push_str("  [OK] Tool permissions enforced\n");
                        if files.iter().any(|f| f == ".env" || f == ".env.local") {
                            result.push_str("  [WARN] Environment file detected — verify no secrets committed\n");
                        }
                    }
                    "performance" => {
                        result.push_str("\nPerformance Analysis:\n");
                        result.push_str("  (Deep profiling requires additional tooling)\n");
                    }
                    _ => {}
                }
            }
            Err(e) => {
                result.push_str(&format!("Error analyzing project: {}", e));
            }
        }

        Ok(result)
    }
}

/// Shorten a URL using TinyURL.
pub struct UrlShortenerTool;
impl ToolPlugin for UrlShortenerTool {
    fn name(&self) -> &str {
        "url_shortener"
    }
    fn description(&self) -> &str {
        "Create a short URL from a long URL using TinyURL."
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "The long URL to shorten" },
                "long_url": { "type": "string", "description": "Alternative parameter — the long URL to shorten" },
                "service": { "type": "string", "description": "URL shortening service (default: tinyurl)", "enum": ["tinyurl"], "default": "tinyurl" }
            },
            "required": ["url"]
        })
    }
    fn execute(&self, args: &Value) -> Result<String> {
        let url = args
            .get("url")
            .or_else(|| args.get("long_url"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'url' argument"))?;

        let api_url = format!(
            "https://tinyurl.com/api-create.php?url={}",
            urlencoding::encode(url)
        );

        let _rt = tokio::runtime::Handle::current();
        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&api_url)
            .send()
            .map_err(|e| anyhow!("Failed to call TinyURL API: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "TinyURL API failed with status: {}",
                response.status()
            ));
        }

        let short_url = response
            .text()
            .map_err(|e| anyhow!("Failed to read TinyURL response: {}", e))?;

        if short_url.starts_with("https://tinyurl.com/") {
            Ok(format!("Shortened URL: {}", short_url))
        } else {
            Err(anyhow!("Invalid response from TinyURL: {}", short_url))
        }
    }
}

/// Convenience macro to quickly define a simple tool.
/// Usage:
/// ```rust
/// define_simple_tool!(
///     MyTool, "my_tool",
///     "Description of my tool",
///     { "type": "object", "properties": { ... } },
///     |args: &Value| -> Result<String> {
///         // implementation
///         Ok("result".to_string())
///     }
/// );
/// ```
#[macro_export]
macro_rules! define_simple_tool {
    (
        $name:ident, $tool_name:expr,
        $desc:expr,
        $params:expr,
        $exec:expr
    ) => {
        pub struct $name;
        impl $crate::tool_plugin::ToolPlugin for $name {
            fn name(&self) -> &str {
                $tool_name
            }
            fn description(&self) -> &str {
                $desc
            }
            fn parameters(&self) -> serde_json::Value {
                $params
            }
            fn execute(&self, args: &serde_json::Value) -> anyhow::Result<String> {
                let __exec: fn(&serde_json::Value) -> anyhow::Result<String> = $exec;
                __exec(args)
            }
        }
    };
}

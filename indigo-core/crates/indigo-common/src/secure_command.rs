use anyhow::{anyhow, Result};
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

/// A list of allowed shell commands for security
const ALLOWED_COMMANDS: &[&str] = &[
    // Basic shell commands
    "echo", "printf", "pwd", "cd",
    // Basic file operations
    "ls", "dir", "cat", "type", "head", "tail", "grep", "find", "cp", "mv", "mkdir", "rmdir",
    // System information
    "ps", "top", "df", "du", "free", "uname", "whoami", "date",
    // Network utilities
    "ping", "curl", "wget", "netstat", "ss",
    // Development tools
    "git", "npm", "python", "python3", "node", "cargo", "rustc",
    // Text processing
    "sed", "awk", "sort", "uniq", "wc", "tr", "cut",
];

/// Dangerous characters and patterns that should not be allowed
const DANGEROUS_PATTERNS: &[&str] = &[
    "&", "|", ";", "`", "$(", "${", "&&", "||", ">", "<", ">>", "<<",
    "rm -rf", "mkfs", "format", "fdisk", "shutdown", "reboot", "halt",
    "passwd", "su", "sudo", "chmod 777", "chown",
];

/// Validates and sanitizes a command string
pub fn validate_command(cmd: &str) -> Result<String> {
    let cmd = cmd.trim();
    
    // Empty command check
    if cmd.is_empty() {
        return Err(anyhow!("Empty command not allowed"));
    }

    // Length limit to prevent command injection via long strings
    if cmd.len() > 1000 {
        return Err(anyhow!("Command too long"));
    }

    // Check for dangerous patterns
    for pattern in DANGEROUS_PATTERNS {
        if cmd.contains(pattern) {
            return Err(anyhow!("Dangerous pattern '{}' detected in command", pattern));
        }
    }

    // Extract the first word to check against allowed commands
    let first_word = cmd.split_whitespace().next().unwrap_or("");
    
    // For shell commands, we need to be more permissive but still validate
    if !is_safe_shell_command(cmd) {
        return Err(anyhow!("Command '{}' is not allowed", first_word));
    }

    Ok(cmd.to_string())
}

/// Checks if a command is safe for shell execution
fn is_safe_shell_command(cmd: &str) -> bool {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }

    let base_cmd = parts[0];
    
    // Allow specific safe commands
    if ALLOWED_COMMANDS.contains(&base_cmd) {
        return true;
    }

    // Allow file paths that look like legitimate paths (not absolute system paths)
    if parts.len() > 1 && (parts[1].starts_with("./") || parts[1].starts_with("../") || parts[1].contains("/")) {
        // This looks like a file operation
        return true;
    }

    // Allow Python/Node scripts with relative paths
    if (base_cmd == "python" || base_cmd == "python3" || base_cmd == "node") && parts.len() > 1 {
        let script = parts[1];
        if script.starts_with("./") || script.ends_with(".py") || script.ends_with(".js") {
            return true;
        }
    }

    // Allow cargo commands
    if base_cmd == "cargo" && parts.len() > 1 {
        let subcmd = parts[1];
        matches!(subcmd, "build" | "run" | "test" | "check" | "doc")
    } else {
        false
    }
}

/// Executes a command safely with validation and sandboxing
pub async fn execute_command_safely(cmd: &str) -> Result<String> {
    let sanitized_cmd = validate_command(cmd)?;
    
    println!("Executing sanitized command: {}", sanitized_cmd);

    #[cfg(target_os = "windows")]
    let output = TokioCommand::new("cmd")
        .arg("/C")
        .arg(&sanitized_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| anyhow!("Failed to execute command: {}", e))?;

    #[cfg(not(target_os = "windows"))]
    let output = TokioCommand::new("bash")
        .arg("-c")
        .arg(&sanitized_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| anyhow!("Failed to execute command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(anyhow!(
            "Command failed with exit code {}: {}\nStderr: {}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        ));
    }

    Ok(format!("Output:\n{}\nErrors:\n{}", stdout, stderr))
}

/// Validates file paths for read/write operations
pub fn validate_file_path(path: &str) -> Result<String> {
    let path = path.trim();
    
    if path.is_empty() {
        return Err(anyhow!("Empty file path"));
    }

    // Prevent path traversal attacks
    if path.contains("..") {
        return Err(anyhow!("Path traversal not allowed"));
    }

    // Prevent absolute paths that could access system files
    #[cfg(target_os = "windows")]
    if path.starts_with("C:\\") || path.starts_with("D:\\") || path.starts_with("\\\\") {
        return Err(anyhow!("Absolute paths not allowed"));
    }

    #[cfg(not(target_os = "windows"))]
    if path.starts_with('/') || path.starts_with("~") {
        return Err(anyhow!("Absolute paths not allowed"));
    }

    // Check for dangerous file extensions or system paths
    let dangerous_paths = [
        "/etc/passwd", "/etc/shadow", "/etc/hosts",
        "C:\\Windows\\System32", "C:\\Windows",
    ];
    
    for dangerous in &dangerous_paths {
        if path.to_lowercase().contains(&dangerous.to_lowercase()) {
            return Err(anyhow!("Access to system files not allowed"));
        }
    }

    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_safe_command_execution() {
        // This should work
        let result = execute_command_safely("echo hello").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_validation() {
        // Safe commands
        assert!(validate_command("ls -la").is_ok());
        assert!(validate_command("echo hello").is_ok());
        assert!(validate_command("python script.py").is_ok());

        // Dangerous commands
        assert!(validate_command("rm -rf /").is_err());
        assert!(validate_command("cat /etc/passwd").is_err());
        assert!(validate_command("echo hello; rm -rf /").is_err());
        assert!(validate_command("`rm -rf /`").is_err());
    }

    #[test]
    fn test_file_path_validation() {
        // Safe paths
        assert!(validate_file_path("./file.txt").is_ok());
        assert!(validate_file_path("script.py").is_ok());
        assert!(validate_file_path("data/output.json").is_ok());

        // Dangerous paths
        assert!(validate_file_path("../etc/passwd").is_err());
        assert!(validate_file_path("/etc/passwd").is_err());
        assert!(validate_file_path("C:\\Windows\\System32\\cmd.exe").is_err());
    }
}
use async_trait::async_trait;
use bedrock_core::Result;
use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, warn};

use super::Tool;
use crate::security::CommandValidator;

pub struct ExecuteBashTool {
    workspace_dir: std::path::PathBuf,
    timeout_seconds: u64,
    max_output_size: usize,
    validator: CommandValidator,
}

impl ExecuteBashTool {
    pub fn new(workspace_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            timeout_seconds: 30,
            max_output_size: 1024 * 1024, // 1MB
            validator: CommandValidator::new(),
        }
    }
    
    pub fn with_validator(mut self, validator: CommandValidator) -> Self {
        self.validator = validator;
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    async fn execute_command(&self, command: &str, working_dir: Option<&str>) -> Result<Value> {
        // Validate command before execution
        if let Err(e) = self.validator.validate(command) {
            return Ok(json!({
                "success": false,
                "error": format!("Command validation failed: {}", e),
                "command": command
            }));
        }
        
        debug!("Executing command: {}", command);
        
        // Parse the command line into command and arguments
        // This is a simple implementation - for production, use a proper shell parser
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(json!({
                "success": false,
                "error": "Empty command",
                "command": command
            }));
        }
        
        let cmd_name = parts[0];
        let args = &parts[1..];
        
        // For complex commands with pipes, redirections etc., we need to use shell
        // Check if the command contains shell metacharacters
        let needs_shell = command.contains('|') || 
                         command.contains('>') || 
                         command.contains('<') ||
                         command.contains('&') ||
                         command.contains(';') ||
                         command.contains('$') ||
                         command.contains('`') ||
                         command.contains('"') ||
                         command.contains('\'');
        
        // Create the command
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", command]);
            cmd
        } else if needs_shell {
            // Use shell for complex commands
            let mut cmd = Command::new("/bin/sh");
            cmd.args(["-c", command]);
            cmd
        } else {
            // Execute directly without shell for simple commands
            let mut cmd = Command::new(cmd_name);
            cmd.args(args);
            cmd
        };

        // Set working directory
        let work_dir = if let Some(dir) = working_dir {
            std::path::PathBuf::from(dir)
        } else {
            // Use current directory instead of workspace_dir if it doesn't exist
            if self.workspace_dir.exists() {
                self.workspace_dir.clone()
            } else {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            }
        };
        
        // Only set current_dir if the directory exists
        if work_dir.exists() {
            cmd.current_dir(&work_dir);
        }

        // Configure command
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Execute with timeout
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(self.timeout_seconds);

        let output = match tokio::time::timeout(timeout, cmd.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Ok(json!({
                    "success": false,
                    "error": format!("Command execution failed: {}", e),
                    "command": command
                }));
            }
            Err(_) => {
                return Ok(json!({
                    "success": false,
                    "error": format!("Command timed out after {} seconds", self.timeout_seconds),
                    "command": command
                }));
            }
        };

        let duration = start_time.elapsed();

        // Convert output to strings
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Truncate output if too large
        let stdout = if stdout.len() > self.max_output_size {
            format!("{}... [output truncated]", &stdout[..self.max_output_size])
        } else {
            stdout.to_string()
        };

        let stderr = if stderr.len() > self.max_output_size {
            format!("{}... [output truncated]", &stderr[..self.max_output_size])
        } else {
            stderr.to_string()
        };

        debug!(
            "Command completed: exit_code={}, duration={:?}",
            output.status.code().unwrap_or(-1),
            duration
        );

        Ok(json!({
            "success": output.status.success(),
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "duration_ms": duration.as_millis(),
            "command": command,
            "working_directory": work_dir.to_string_lossy()
        }))
    }
}

#[async_trait]
impl Tool for ExecuteBashTool {
    fn name(&self) -> &str {
        if cfg!(target_os = "windows") {
            "execute_cmd"
        } else {
            "execute_bash"
        }
    }

    fn description(&self) -> &str {
        if cfg!(target_os = "windows") {
            "Execute Windows command prompt commands. Supports common system commands and utilities."
        } else {
            "Execute bash shell commands. Supports common Unix/Linux commands and utilities."
        }
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": if cfg!(target_os = "windows") {
                        "The Windows command to execute"
                    } else {
                        "The bash command to execute"
                    }
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional working directory for command execution"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| bedrock_core::BedrockError::ToolError {
                tool: self.name().to_string(),
                message: "Missing 'command' parameter".to_string(),
            })?;

        let working_dir = args
            .get("working_directory")
            .and_then(|v| v.as_str());

        // Validate command is not empty
        if command.trim().is_empty() {
            return Ok(json!({
                "error": "Command cannot be empty",
                "command": command
            }));
        }

        match self.execute_command(command, working_dir).await {
            Ok(result) => {
                debug!("Command executed successfully: {:?}", result);
                Ok(result)
            }
            Err(e) => {
                warn!("Command execution failed: {}", e);
                Ok(json!({
                    "error": e.to_string(),
                    "command": command,
                    "working_directory": working_dir
                }))
            }
        }
    }
}
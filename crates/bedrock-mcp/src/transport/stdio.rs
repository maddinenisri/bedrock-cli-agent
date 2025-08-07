//! Stdio transport implementation for process-based MCP servers

use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use serde_json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info};

use crate::types::{JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};
use super::Transport;

/// Stdio transport for process-based MCP servers
pub struct StdioTransport {
    /// Child process handle
    process: Arc<Mutex<Option<Child>>>,
    
    /// Process stdin for sending data
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    
    /// Channel for receiving responses
    response_rx: Arc<Mutex<mpsc::Receiver<JsonRpcResponse>>>,
    
    /// Process metadata
    command: String,
    args: Vec<String>,
    #[allow(dead_code)]
    env: HashMap<String, String>,
    
    /// Connection state
    connected: Arc<RwLock<bool>>,
}

impl std::fmt::Debug for StdioTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport")
            .field("command", &self.command)
            .field("args", &self.args)
            .finish()
    }
}

impl StdioTransport {
    /// Create a new stdio transport
    pub async fn new(
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        _timeout: u64, // Will be used for future timeout handling
    ) -> Result<Self> {
        info!("Starting MCP server via stdio: {} {:?}", command, args);
        
        // Build the command with environment variables
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true); // Ensure process is killed when transport is dropped
        
        // Set environment variables
        for (key, value) in &env {
            // Resolve environment variable values
            let resolved_value = resolve_env_value(value);
            cmd.env(key, resolved_value);
        }
        
        // Spawn the process
        let mut child = cmd.spawn()
            .map_err(|e| BedrockError::McpError(format!("Failed to spawn MCP server process: {}", e)))?;
        
        // Get process streams
        let stdin = child.stdin.take()
            .ok_or_else(|| BedrockError::McpError("Failed to get process stdin".into()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| BedrockError::McpError("Failed to get process stdout".into()))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| BedrockError::McpError("Failed to get process stderr".into()))?;
        
        // Create response channel
        let (response_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(100);
        
        // Start stdout reader task
        let response_tx_clone = response_tx.clone();
        let connected = Arc::new(RwLock::new(true));
        let connected_clone = connected.clone();
        
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF reached, process has ended
                        info!("MCP server process stdout closed");
                        *connected_clone.write().await = false;
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            debug!("Received from MCP server: {}", trimmed);
                            
                            // Try to parse as JSON-RPC response
                            match serde_json::from_str::<JsonRpcResponse>(trimmed) {
                                Ok(response) => {
                                    if let Err(e) = response_tx_clone.send(response).await {
                                        error!("Failed to send response through channel: {}", e);
                                    }
                                }
                                Err(e) => {
                                    debug!("Non-JSON-RPC message from server: {} - {}", trimmed, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading from MCP server stdout: {}", e);
                        *connected_clone.write().await = false;
                        break;
                    }
                }
            }
        });
        
        // Start stderr reader task (for logging)
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            debug!("MCP server stderr: {}", trimmed);
                        }
                    }
                    Err(e) => {
                        error!("Error reading from MCP server stderr: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(Self {
            process: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(Mutex::new(Some(stdin))),
            response_rx: Arc::new(Mutex::new(response_rx)),
            command,
            args,
            env,
            connected,
        })
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<()> {
        let mut stdin_guard = self.stdin.lock().await;
        if let Some(stdin) = stdin_guard.as_mut() {
            let json = serde_json::to_string(&request)
                .map_err(|e| BedrockError::SerializationError(e))?;
            
            debug!("Sending to MCP server: {}", json);
            stdin.write_all(json.as_bytes()).await
                .map_err(|e| BedrockError::McpError(format!("Failed to write to stdin: {}", e)))?;
            stdin.write_all(b"\n").await
                .map_err(|e| BedrockError::McpError(format!("Failed to write newline: {}", e)))?;
            stdin.flush().await
                .map_err(|e| BedrockError::McpError(format!("Failed to flush stdin: {}", e)))?;
            
            Ok(())
        } else {
            Err(BedrockError::McpError("Process stdin not available".into()))
        }
    }
    
    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let mut stdin_guard = self.stdin.lock().await;
        if let Some(stdin) = stdin_guard.as_mut() {
            let json = serde_json::to_string(&notification)
                .map_err(|e| BedrockError::SerializationError(e))?;
            
            debug!("Sending notification to MCP server: {}", json);
            stdin.write_all(json.as_bytes()).await
                .map_err(|e| BedrockError::McpError(format!("Failed to write to stdin: {}", e)))?;
            stdin.write_all(b"\n").await
                .map_err(|e| BedrockError::McpError(format!("Failed to write newline: {}", e)))?;
            stdin.flush().await
                .map_err(|e| BedrockError::McpError(format!("Failed to flush stdin: {}", e)))?;
            
            Ok(())
        } else {
            Err(BedrockError::McpError("Process stdin not available".into()))
        }
    }
    
    async fn receive_response(&mut self) -> Result<Option<JsonRpcResponse>> {
        let mut rx_guard = self.response_rx.lock().await;
        
        // Use try_recv to avoid blocking
        match rx_guard.try_recv() {
            Ok(response) => Ok(Some(response)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(BedrockError::McpError("Response channel disconnected".into()))
            }
        }
    }
    
    async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    async fn close(&mut self) -> Result<()> {
        info!("Closing stdio transport");
        
        // Close stdin
        if let Some(mut stdin) = self.stdin.lock().await.take() {
            let _ = stdin.shutdown().await;
        }
        
        // Kill the process
        if let Some(mut child) = self.process.lock().await.take() {
            match child.kill().await {
                Ok(_) => info!("MCP server process terminated"),
                Err(e) => error!("Failed to kill MCP server process: {}", e),
            }
        }
        
        *self.connected.write().await = false;
        Ok(())
    }
}

/// Resolve environment variable values
fn resolve_env_value(value: &str) -> String {
    if value.starts_with("${") && value.ends_with("}") {
        let inner = &value[2..value.len()-1];
        
        // Handle ${VAR:-default} pattern
        if let Some((var_name, default)) = inner.split_once(":-") {
            std::env::var(var_name).unwrap_or_else(|_| default.to_string())
        } else {
            std::env::var(inner).unwrap_or_else(|_| value.to_string())
        }
    } else {
        value.to_string()
    }
}
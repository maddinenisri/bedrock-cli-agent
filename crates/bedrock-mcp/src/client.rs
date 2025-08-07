//! MCP client implementation

use bedrock_core::{BedrockError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio::task::JoinHandle;
use tracing::{debug, info, error};

use crate::config::McpServerConfig;
use crate::transport::Transport;
use crate::types::{
    ClientCapabilities, ClientInfo, ContentItem, InitializeParams, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, ListToolsResult, McpTool,
    ToolCallParams, ToolCallResult,
};

/// MCP client for communicating with an MCP server
pub struct McpClient {
    /// Server name for identification
    name: String,
    
    /// Transport for communication (shared with response handler)
    transport: Arc<RwLock<Box<dyn Transport>>>,
    
    /// Request ID counter
    request_id: Arc<AtomicU64>,
    
    /// Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>,
    
    /// Response handler task
    response_handler: Option<JoinHandle<()>>,
    
    /// Server capabilities (set after initialization)
    capabilities: Option<InitializeResult>,
}

impl McpClient {
    /// Create a new MCP client
    pub async fn new(name: String, config: McpServerConfig) -> Result<Self> {
        let transport_config = config.to_transport_config();
        let transport = transport_config.create_transport().await?;
        let transport = Arc::new(RwLock::new(transport));
        
        let pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>> = 
            Arc::new(Mutex::new(HashMap::new()));
        
        // Start response handler task with shared transport
        let pending_clone = pending_requests.clone();
        let transport_clone = transport.clone();
        
        let response_handler = tokio::spawn(async move {
            loop {
                // Try to receive a response (release lock quickly)
                let response = {
                    let mut transport = transport_clone.write().await;
                    let resp = transport.receive_response().await;
                    drop(transport); // Explicitly drop lock
                    resp
                };
                
                match response {
                    Ok(Some(response)) => {
                        debug!("Received response with id: {}", response.id);
                        
                        let mut pending = pending_clone.lock().await;
                        if let Some(sender) = pending.remove(&response.id) {
                            if sender.send(response).is_err() {
                                debug!("Failed to send response to waiting request");
                            }
                        } else {
                            debug!("No pending request for response id: {}", response.id);
                        }
                    }
                    Ok(None) => {
                        // No response available, wait a bit
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(e) => {
                        error!("Error receiving response: {}", e);
                        // Don't break on error, just log and continue
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        Ok(Self {
            name,
            transport,
            request_id: Arc::new(AtomicU64::new(1)),
            pending_requests,
            response_handler: Some(response_handler),
            capabilities: None,
        })
    }
    
    /// Get the next request ID
    fn next_request_id(&self) -> String {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        format!("{}", id)
    }
    
    /// Initialize the MCP connection
    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        info!("Initializing MCP client: {}", self.name);
        
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo {
                name: "bedrock-cli-agent".to_string(),
                version: "0.1.0".to_string(),
            },
        };
        
        let request = JsonRpcRequest::new(
            self.next_request_id(),
            "initialize".to_string(),
            Some(serde_json::to_value(params)?),
        );
        
        let response = self.send_request(request).await?;
        
        if let Some(error) = response.error {
            return Err(BedrockError::McpError(format!(
                "Failed to initialize MCP connection: {}",
                error.message
            )));
        }
        
        let result: InitializeResult = serde_json::from_value(
            response.result.ok_or_else(|| {
                BedrockError::McpError("Initialize response missing result".into())
            })?
        )?;
        
        info!(
            "MCP client '{}' initialized with protocol version: {}",
            self.name, result.protocol_version
        );
        
        if let Some(ref server_info) = result.server_info {
            info!(
                "Connected to MCP server: {} v{}",
                server_info.name, server_info.version
            );
        }
        
        // Send initialized notification (critical for some servers)
        info!("Preparing to send notifications/initialized");
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: Some(serde_json::json!({})),
        };
        
        info!("Sending notification to transport");
        {
            let mut transport = self.transport.write().await;
            transport.send_notification(notification).await?;
        }
        
        info!("Sent notifications/initialized to server");
        
        // Allow server time to become ready after initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        self.capabilities = Some(result.clone());
        Ok(result)
    }
    
    /// List available tools from the MCP server
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        debug!("Listing tools from MCP server: {}", self.name);
        
        let request = JsonRpcRequest::new(
            self.next_request_id(),
            "tools/list".to_string(),
            None,
        );
        
        let response = self.send_request(request).await?;
        
        if let Some(error) = response.error {
            return Err(BedrockError::McpError(format!(
                "Failed to list tools: {}",
                error.message
            )));
        }
        
        let result: ListToolsResult = serde_json::from_value(
            response.result.ok_or_else(|| {
                BedrockError::McpError("List tools response missing result".into())
            })?
        )?;
        
        info!(
            "Discovered {} tools from MCP server '{}'",
            result.tools.len(),
            self.name
        );
        
        Ok(result.tools)
    }
    
    /// Call a tool on the MCP server
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Vec<ContentItem>> {
        debug!("Calling MCP tool '{}' on server '{}'", name, self.name);
        
        let params = ToolCallParams {
            name: name.to_string(),
            arguments,
        };
        
        let request = JsonRpcRequest::new(
            self.next_request_id(),
            "tools/call".to_string(),
            Some(serde_json::to_value(params)?),
        );
        
        let response = self.send_request(request).await?;
        
        if let Some(error) = response.error {
            return Err(BedrockError::McpError(format!(
                "Tool '{}' execution failed: {}",
                name, error.message
            )));
        }
        
        let result: ToolCallResult = serde_json::from_value(
            response.result.ok_or_else(|| {
                BedrockError::McpError(format!("Tool '{}' response missing result", name))
            })?
        )?;
        
        if result.is_error.unwrap_or(false) {
            return Err(BedrockError::McpError(format!(
                "Tool '{}' returned an error",
                name
            )));
        }
        
        Ok(result.content)
    }
    
    /// Send a request and wait for response
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let (tx, rx) = oneshot::channel();
        let request_id = request.id.clone();
        
        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }
        
        // Send the request
        {
            let mut transport = self.transport.write().await;
            transport.send_request(request).await?;
        }
        
        // Wait for response with timeout
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            rx
        ).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Remove from pending if still there
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                Err(BedrockError::McpError("Request cancelled".into()))
            }
            Err(_) => {
                // Remove from pending if still there
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                Err(BedrockError::McpError("Request timed out".into()))
            }
        }
    }
    
    /// Check if the client is connected
    pub async fn is_connected(&self) -> bool {
        let transport = self.transport.read().await;
        transport.is_connected().await
    }
    
    /// Close the client connection
    pub async fn close(&mut self) -> Result<()> {
        debug!("Closing MCP client: {}", self.name);
        
        // Cancel response handler
        if let Some(handler) = self.response_handler.take() {
            handler.abort();
        }
        
        // Close transport
        let mut transport = self.transport.write().await;
        transport.close().await?;
        
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Cancel response handler if still running
        if let Some(handler) = self.response_handler.take() {
            handler.abort();
        }
    }
}
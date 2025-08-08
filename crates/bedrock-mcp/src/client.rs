//! MCP client implementation

use bedrock_core::{BedrockError, Result};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

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
    
    /// Transport for communication
    transport: Arc<RwLock<Box<dyn Transport>>>,
    
    /// Request ID counter
    request_id: Arc<AtomicU64>,
    
    /// Server capabilities (set after initialization)
    capabilities: Option<InitializeResult>,
    
    /// Cached tools from the server
    tools_cache: Vec<McpTool>,
    
    /// Timeout duration for requests (in milliseconds)
    timeout_ms: u64,
}

impl McpClient {
    /// Create a new MCP client
    pub async fn new(name: String, config: McpServerConfig) -> Result<Self> {
        let transport_config = config.to_transport_config();
        let transport = transport_config.create_transport().await?;
        let transport = Arc::new(RwLock::new(transport));
        
        // Extract timeout from config
        let timeout_ms = match &config {
            McpServerConfig::Stdio { timeout, .. } => *timeout,
            McpServerConfig::Sse { timeout, .. } => *timeout,
        };
        
        Ok(Self {
            name,
            transport,
            request_id: Arc::new(AtomicU64::new(1)),
            capabilities: None,
            tools_cache: Vec::new(),
            timeout_ms,
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
    
    /// Get cached tools (populated during initialization)
    pub async fn get_tools(&self) -> Vec<McpTool> {
        self.tools_cache.clone()
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
        
        // Cache the tools
        self.tools_cache = result.tools.clone();
        
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
    
    /// Send a request and wait for response with direct correlation
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let request_id = request.id.clone();
        
        // Send the request
        {
            let mut transport = self.transport.write().await;
            transport.send_request(request).await?;
        }
        
        // Wait for response with timeout and correlation
        let timeout_duration = Duration::from_millis(self.timeout_ms);
        
        timeout(timeout_duration, self.wait_for_response(request_id.clone())).await
            .map_err(|_| BedrockError::McpError(
                format!("Request {} timed out after {}ms", request_id, self.timeout_ms)
            ))?
    }
    
    /// Wait for a specific response by ID
    async fn wait_for_response(&mut self, request_id: String) -> Result<JsonRpcResponse> {
        let start = std::time::Instant::now();
        let max_wait = Duration::from_millis(self.timeout_ms);
        
        loop {
            // Check for timeout
            if start.elapsed() > max_wait {
                return Err(BedrockError::McpError(
                    format!("Timeout waiting for response to request {}", request_id)
                ));
            }
            
            // Try to receive response
            let mut transport = self.transport.write().await;
            if let Some(response) = transport.receive_response().await? {
                if response.id == request_id {
                    return Ok(response);
                }
                // If not our response, log and continue
                // This could happen if responses arrive out of order
                warn!("Received response for different request: {} (expected: {})", 
                      response.id, request_id);
            }
            
            // Release lock and wait briefly before retrying
            drop(transport);
            tokio::time::sleep(Duration::from_millis(10)).await;
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
        
        // Close transport
        let mut transport = self.transport.write().await;
        transport.close().await?;
        
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Nothing to clean up with simplified design
        debug!("Dropping MCP client: {}", self.name);
    }
}
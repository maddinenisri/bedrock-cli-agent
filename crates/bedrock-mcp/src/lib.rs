use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use bedrock_tools::{Tool, ToolRegistry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: TransportType,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Sse,
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&mut self, message: McpMessage) -> Result<()>;
    async fn recv(&mut self) -> Result<McpMessage>;
    async fn close(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpMessage {
    Request(McpRequest),
    Response(McpResponse),
    Notification(McpNotification),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub id: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub id: String,
    pub result: Option<Value>,
    pub error: Option<McpError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpNotification {
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

pub struct McpClient {
    transport: Box<dyn Transport>,
    pending_requests: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<McpResponse>>>>,
}

impl McpClient {
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let request = McpRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: "initialize".to_string(),
            params: serde_json::json!({
                "protocolVersion": "1.0",
                "capabilities": {}
            }),
        };

        let response = self.call_internal(request).await?;
        
        if response.error.is_some() {
            return Err(BedrockError::McpError("Failed to initialize MCP connection".into()));
        }

        info!("MCP client initialized successfully");
        Ok(())
    }

    pub async fn list_tools(&mut self) -> Result<Vec<ToolInfo>> {
        let request = McpRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: "tools/list".to_string(),
            params: serde_json::json!({}),
        };

        let response = self.call_internal(request).await?;
        
        if let Some(error) = response.error {
            return Err(BedrockError::McpError(format!("Failed to list tools: {}", error.message)));
        }

        let tools: Vec<ToolInfo> = serde_json::from_value(
            response.result.unwrap_or_else(|| serde_json::json!([]))
        ).map_err(BedrockError::SerializationError)?;

        Ok(tools)
    }

    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        let request = McpRequest {
            id: uuid::Uuid::new_v4().to_string(),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": name,
                "arguments": args
            }),
        };

        let response = self.call_internal(request).await?;
        
        if let Some(error) = response.error {
            return Err(BedrockError::McpError(format!("Tool execution failed: {}", error.message)));
        }

        Ok(response.result.unwrap_or(serde_json::json!(null)))
    }

    async fn call_internal(&mut self, request: McpRequest) -> Result<McpResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let request_id = request.id.clone();
        
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        self.transport.send(McpMessage::Request(request)).await?;
        
        let response = rx.await
            .map_err(|_| BedrockError::McpError("Request cancelled".into()))?;
        
        Ok(response)
    }

    pub async fn close(&mut self) -> Result<()> {
        self.transport.close().await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct McpToolAdapter {
    mcp_client: Arc<Mutex<McpClient>>,
    tool_info: ToolInfo,
}

impl McpToolAdapter {
    pub fn new(mcp_client: Arc<Mutex<McpClient>>, tool_info: ToolInfo) -> Self {
        Self {
            mcp_client,
            tool_info,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool_info.name
    }

    fn description(&self) -> &str {
        &self.tool_info.description
    }

    fn schema(&self) -> Value {
        self.tool_info.input_schema.clone()
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let mut client = self.mcp_client.lock().await;
        client.call_tool(&self.tool_info.name, args).await
    }
}

pub struct McpManager {
    clients: Vec<Arc<Mutex<McpClient>>>,
    tool_registry: Arc<ToolRegistry>,
}

impl McpManager {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            clients: Vec::new(),
            tool_registry,
        }
    }

    pub async fn add_client(&mut self, mut client: McpClient) -> Result<()> {
        client.initialize().await?;
        
        let tools = client.list_tools().await?;
        let client_arc = Arc::new(Mutex::new(client));
        
        for tool_info in tools {
            let adapter = McpToolAdapter::new(Arc::clone(&client_arc), tool_info.clone());
            self.tool_registry.register(adapter)?;
            debug!("Registered MCP tool: {}", tool_info.name);
        }
        
        self.clients.push(client_arc);
        Ok(())
    }

    pub async fn close_all(&mut self) -> Result<()> {
        for client in &self.clients {
            let mut c = client.lock().await;
            if let Err(e) = c.close().await {
                warn!("Error closing MCP client: {}", e);
            }
        }
        self.clients.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_message_serialization() {
        let request = McpRequest {
            id: "test-123".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({"key": "value"}),
        };

        let message = McpMessage::Request(request);
        let json = serde_json::to_string(&message).unwrap();
        let parsed: McpMessage = serde_json::from_str(&json).unwrap();
        
        match parsed {
            McpMessage::Request(req) => {
                assert_eq!(req.id, "test-123");
                assert_eq!(req.method, "test");
            }
            _ => panic!("Wrong message type"),
        }
    }
}
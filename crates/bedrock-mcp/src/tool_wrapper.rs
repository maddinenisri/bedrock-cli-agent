//! Wrapper for MCP tools to implement the bedrock-tools Tool trait

use async_trait::async_trait;
use bedrock_core::Result;
use bedrock_tools::Tool;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::client::McpClient;
use crate::conversions::process_mcp_response;
use crate::types::{ContentItem, McpTool};

/// Wrapper for MCP tools to implement our Tool trait
pub struct McpToolWrapper {
    /// Tool definition from MCP server
    tool_def: McpTool,
    
    /// MCP client for executing the tool
    client: Arc<RwLock<McpClient>>,
    
    /// Server name (for tracking, not exposed in tool name)
    server_name: String,
}

impl McpToolWrapper {
    /// Create a new MCP tool wrapper
    pub fn new(tool_def: McpTool, client: Arc<RwLock<McpClient>>, server_name: String) -> Self {
        Self {
            tool_def,
            client,
            server_name,
        }
    }
    
    /// Get the server name this tool belongs to
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        // Use simple tool name without server prefix for Bedrock compatibility
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn schema(&self) -> Value {
        // Return raw schema directly - Bedrock handles it fine
        // Following reference project pattern (no cleaning needed)
        self.tool_def.input_schema.clone()
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        debug!(
            "Executing MCP tool '{}' from server '{}'",
            self.tool_def.name, self.server_name
        );
        
        // Call the tool through MCP client
        let mut client = self.client.write().await;
        match client.call_tool(&self.tool_def.name, args).await {
            Ok(content_items) => {
                // Process content items
                let mut text_content = Vec::new();
                let mut images = Vec::new();
                
                for item in content_items {
                    match item {
                        ContentItem::Text { text } => {
                            text_content.push(text);
                        }
                        ContentItem::Image { data, mime_type } => {
                            images.push(json!({
                                "type": "image",
                                "data": data,
                                "mime_type": mime_type
                            }));
                        }
                    }
                }
                
                // Use the centralized response processing
                let mut response = process_mcp_response(text_content, images);
                
                // Add metadata for debugging
                if let Value::Object(ref mut map) = response {
                    map.insert("server".to_string(), json!(self.server_name));
                    map.insert("tool".to_string(), json!(self.tool_def.name));
                }
                
                Ok(response)
            }
            Err(e) => {
                error!("MCP tool execution failed: {}", e);
                
                // Return error in a format that Bedrock can understand
                Ok(json!({
                    "error": e.to_string(),
                    "success": false,
                    "server": self.server_name,
                    "tool": self.tool_def.name
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServerConfig;
    
    #[tokio::test]
    async fn test_tool_wrapper_name() {
        // Create a mock tool definition
        let tool_def = McpTool {
            name: "read_file".to_string(),
            description: "Read contents of a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string"
                    }
                }
            }),
        };
        
        // Create a dummy config for the client
        let config = McpServerConfig::Stdio {
            command: "echo".to_string(),
            args: vec![],
            env: Default::default(),
            timeout: 30000,
            disabled: false,
            health_check: None,
            restart_policy: None,
        };
        
        // Note: In a real test, we'd use a mock client
        // For now, we'll just test the wrapper's basic properties
        let client = Arc::new(RwLock::new(
            McpClient::new("test".to_string(), config).await.unwrap()
        ));
        
        let wrapper = McpToolWrapper::new(
            tool_def.clone(),
            client,
            "test-server".to_string(),
        );
        
        // Verify the wrapper exposes the correct name (without server prefix)
        assert_eq!(wrapper.name(), "read_file");
        assert_eq!(wrapper.description(), "Read contents of a file");
        assert_eq!(wrapper.server_name(), "test-server");
    }
}
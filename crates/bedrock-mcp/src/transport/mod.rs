//! Transport layer for MCP communication
//! 
//! Provides transport abstractions for MCP communication with support for:
//! - Stdio (process-based) transport
//! - SSE (Server-Sent Events) transport

use async_trait::async_trait;
use bedrock_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

use crate::types::{JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};

pub mod stdio;
pub mod sse;

pub use stdio::StdioTransport;
pub use sse::SseTransport;

/// Transport trait for MCP communication
#[async_trait]
pub trait Transport: Send + Sync + Debug {
    /// Send a JSON-RPC request
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<()>;
    
    /// Send a JSON-RPC notification (no response expected)
    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()>;
    
    /// Receive a JSON-RPC response
    async fn receive_response(&mut self) -> Result<Option<JsonRpcResponse>>;
    
    /// Check if transport is connected
    async fn is_connected(&self) -> bool;
    
    /// Close the transport connection
    async fn close(&mut self) -> Result<()>;
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransportConfig {
    /// Stdio-based transport (process communication)
    Stdio {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
    
    /// SSE-based transport (HTTP Server-Sent Events)
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
}

fn default_timeout() -> u64 {
    30000 // 30 seconds default
}

impl TransportConfig {
    /// Create a transport instance from configuration
    pub async fn create_transport(&self) -> Result<Box<dyn Transport>> {
        match self {
            TransportConfig::Stdio { command, args, env, timeout } => {
                let transport = StdioTransport::new(
                    command.clone(),
                    args.clone(),
                    env.clone(),
                    *timeout,
                ).await?;
                Ok(Box::new(transport))
            }
            TransportConfig::Sse { url, headers, timeout } => {
                let transport = SseTransport::new(
                    url.clone(),
                    headers.clone(),
                    *timeout,
                ).await?;
                Ok(Box::new(transport))
            }
        }
    }
    
    /// Get transport type as string
    pub fn transport_type(&self) -> &str {
        match self {
            TransportConfig::Stdio { .. } => "stdio",
            TransportConfig::Sse { .. } => "sse",
        }
    }
}
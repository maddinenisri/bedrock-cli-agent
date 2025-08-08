//! Model Context Protocol (MCP) integration for external tool discovery
//! 
//! This module provides MCP client support with stdio and SSE transports,
//! enabling connection to MCP servers that provide additional tools.

pub mod client;
pub mod config;
pub mod conversions;
pub mod manager;
pub mod tool_wrapper;
pub mod transport;
pub mod types;

// Re-export key types for convenience
pub use client::McpClient;
pub use config::{McpConfig, McpServerConfig, HealthCheckConfig, RestartPolicy, BackoffStrategy};
pub use conversions::{process_mcp_response, validate_json_for_mcp};
pub use manager::McpManager;
pub use tool_wrapper::McpToolWrapper;
pub use types::{McpTool, ContentItem, JsonRpcRequest, JsonRpcResponse};
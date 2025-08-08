//! Test MCP SSE transport with Redux API server
//! 
//! Prerequisites:
//! 1. Ensure the Redux API server is running on http://localhost:8080
//! 2. The server should be configured to accept the provided token
//! 
//! This example demonstrates:
//! - SSE transport configuration
//! - Connecting to Redux DevTools MCP server
//! - Discovering Redux-specific tools
//! - Tool execution with SSE transport

use anyhow::Result;
use bedrock_mcp::{McpConfig, McpManager};
use bedrock_tools::ToolRegistry;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    
    info!("Starting MCP SSE Redux DevTools test");
    
    // Load configuration from YAML file
    let config_path = "examples/mcp-sse-redux.yaml";
    let config = match McpConfig::load_from_file(config_path).await {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not load config file: {}", e);
            info!("Creating configuration programmatically...");
            
            // Create config programmatically as fallback
            let mut config = McpConfig::new();
            let server_config = bedrock_mcp::McpServerConfig::Sse {
                transport_type: Some("sse".to_string()),
                url: "http://localhost:8080".to_string(),
                headers: {
                    let mut h = std::collections::HashMap::new();
                    h.insert("token".to_string(), "gAAAAABok5IS5q-OKYBO8UZYT7DBcX5PTcrEJalYAGWRpg3J-5WfOmjw2_haU2nD59d-pz7IgFjQo7p4-ILMCO4zginlC-4GRXDC3PRpa_mz67tggCJsrQqeYnXd0oK0zRuPdY_38TVKs4ZGYfoApHYnSlwjiuPbznOxpKrOhl274KsOJDfToHDEUZTMyZKb-et6r7YcCRrM".to_string());
                    h
                },
                timeout: 30000,
                disabled: false,
                health_check: Some(bedrock_mcp::HealthCheckConfig {
                    interval: 30,
                    timeout: 10,
                    max_failures: 3,
                }),
                restart_policy: Some(bedrock_mcp::RestartPolicy {
                    max_retries: 3,
                    initial_delay: 5,
                    max_delay: 60,
                    backoff: bedrock_mcp::BackoffStrategy::Exponential,
                }),
            };
            config.mcp_servers.insert("reduxApi".to_string(), server_config);
            config
        }
    };
    
    info!("Configuration loaded with {} servers", config.mcp_servers.len());
    
    // Create tool registry
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Create MCP manager
    let mut manager = McpManager::new(tool_registry.clone());
    
    // Add servers from config
    manager.add_servers_from_config(config.mcp_servers.clone()).await?;
    
    // Start all servers
    info!("Starting MCP servers...");
    match manager.start_servers(vec![]).await {
        Ok(()) => {
            info!("Successfully started MCP servers");
        }
        Err(e) => {
            warn!("Failed to start MCP servers: {}", e);
            info!("Note: This is expected if Redux API server is not running");
            info!("Ensure the server is running on http://localhost:8080");
            info!("The server should accept the authentication token provided");
            return Ok(());
        }
    }
    
    // List running servers
    let servers = manager.list_servers().await;
    info!("Running servers: {:?}", servers);
    
    // Get server info
    for server_name in &servers {
        if let Some((tools, connected)) = manager.get_server_info(server_name).await {
            info!("Server '{}': connected={}, tools={}", server_name, connected, tools.len());
            for tool_name in &tools {
                info!("  - {}", tool_name);
            }
            
            // Try to execute a Redux-specific tool if available
            if tools.contains(&"getState".to_string()) {
                info!("Testing Redux getState tool...");
                
                let registry = &tool_registry;
                if let Some(tool) = registry.get("getState") {
                    match tool.execute(json!({})).await {
                        Ok(result) => {
                            info!("Redux state retrieved: {:?}", result);
                        }
                        Err(e) => {
                            info!("Could not get Redux state (expected if no store connected): {}", e);
                        }
                    }
                }
            }
            
            if tools.contains(&"dispatch".to_string()) {
                info!("Redux dispatch tool is available for dispatching actions");
            }
            
            if tools.contains(&"subscribe".to_string()) {
                info!("Redux subscribe tool is available for monitoring state changes");
            }
        }
    }
    
    // Shutdown servers
    info!("Shutting down MCP servers...");
    manager.stop_all().await?;
    
    info!("✅ MCP SSE Redux test completed successfully!");
    
    Ok(())
}
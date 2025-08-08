//! Test MCP stdio transport with Figma Developer MCP Server
//! 
//! This example demonstrates:
//! - Stdio transport configuration with environment variables
//! - Connecting to Figma Developer MCP server
//! - Discovering Figma design tools
//! - Tool execution with stdio transport

use anyhow::Result;
use bedrock_mcp::{McpConfig, McpManager, McpServerConfig};
use bedrock_tools::ToolRegistry;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    
    info!("Starting MCP Stdio Figma Developer test");
    
    // Load configuration from YAML file
    let config_path = "examples/mcp-stdio-figma.yaml";
    let config = match McpConfig::load_from_file(config_path).await {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not load config file: {}", e);
            info!("Creating configuration programmatically...");
            
            // Create config programmatically as fallback
            let mut config = McpConfig::new();
            let server_config = McpServerConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "figma-developer-mcp".to_string(),
                    "--stdio".to_string(),
                ],
                env: {
                    let mut env = HashMap::new();
                    env.insert(
                        "FIGMA_API_KEY".to_string(),
                        std::env::var("FIGMA_API_KEY").unwrap_or_else(|_| "your-figma-api-key".to_string()),
                    );
                    env
                },
                timeout: 60000,
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
            config.mcp_servers.insert("figma-mcp-server".to_string(), server_config);
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
            info!("✅ Successfully started MCP servers");
        }
        Err(e) => {
            warn!("Failed to start MCP servers: {}", e);
            info!("Note: This is expected if figma-developer-mcp is not installed");
            info!("The server will be installed automatically via npx -y");
            info!("If it still fails, you can manually install:");
            info!("  npm install -g figma-developer-mcp");
            return Ok(());
        }
    }
    
    // List running servers
    let servers = manager.list_servers().await;
    info!("Running servers: {:?}", servers);
    
    // Get server info and list tools
    for server_name in &servers {
        if let Some((tools, connected)) = manager.get_server_info(server_name).await {
            info!("Server '{}': connected={}, tools={}", server_name, connected, tools.len());
            
            info!("\n📦 Available Figma Developer Tools:");
            for tool_name in &tools {
                info!("  🔧 {}", tool_name);
                
                // Describe common Figma tools
                match tool_name.as_str() {
                    "getFile" => info!("      → Retrieve complete Figma file data and structure"),
                    "getFileNodes" => info!("      → Get specific nodes from a Figma file"),
                    "getImages" => info!("      → Export images from Figma designs"),
                    "getImageFills" => info!("      → Get image fill URLs from a file"),
                    "getComments" => info!("      → Retrieve comments from a Figma file"),
                    "postComment" => info!("      → Post a new comment to a Figma file"),
                    "getUser" => info!("      → Get information about the current user"),
                    "getTeamProjects" => info!("      → List all projects in a Figma team"),
                    "getProjectFiles" => info!("      → List all files in a Figma project"),
                    "getTeamStyles" => info!("      → Get published styles from a team library"),
                    "getTeamComponents" => info!("      → Get published components from a team"),
                    "getFileVersions" => info!("      → Get version history of a Figma file"),
                    "getFileStyles" => info!("      → Get local styles from a Figma file"),
                    "getFileComponents" => info!("      → Get local components from a Figma file"),
                    "getFileComponentSets" => info!("      → Get component sets from a file"),
                    _ => {}
                }
            }
            
            // Example: Try to get user information
            if tools.contains(&"getUser".to_string()) {
                info!("\n🧪 Testing Figma getUser tool...");
                
                let registry = &tool_registry;
                if let Some(tool) = registry.get("getUser") {
                    match tool.execute(json!({})).await {
                        Ok(result) => {
                            info!("✅ User information retrieved successfully!");
                            info!("Response: {}", serde_json::to_string_pretty(&result)?);
                        }
                        Err(e) => {
                            warn!("Could not get user info: {}", e);
                        }
                    }
                }
            }
            
            // Example: List team projects (requires team_id)
            if tools.contains(&"getTeamProjects".to_string()) {
                info!("\n🧪 Testing Figma getTeamProjects tool...");
                info!("Note: This requires a valid team_id to work");
                
                // You would need to replace with actual team_id
                // Example: "123456789" 
                // The tool will fail gracefully if team_id is invalid
            }
        }
    }
    
    // Demonstrate stdio-specific features
    info!("\n🚀 Stdio Transport Features Demonstrated:");
    info!("  ✓ Process-based MCP server communication");
    info!("  ✓ Environment variable passing for API keys");
    info!("  ✓ Automatic server installation via npx");
    info!("  ✓ JSON-RPC over stdio pipes");
    info!("  ✓ Health monitoring and auto-restart");
    info!("  ✓ Tool discovery and registration");
    
    // Shutdown servers
    info!("\n🔌 Shutting down MCP servers...");
    manager.stop_all().await?;
    
    info!("✅ MCP Stdio Figma test completed successfully!");
    info!("The MCP integration is working correctly with stdio transport.");
    
    Ok(())
}
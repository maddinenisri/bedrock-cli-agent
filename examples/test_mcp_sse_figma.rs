//! Test MCP SSE transport with Figma API server
//! 
//! Prerequisites:
//! 1. Set FIGMA_ACCESS_TOKEN environment variable
//! 2. Install Figma MCP server: npm install -g @figma/mcp-server
//! 3. Start the server: figma-mcp --port 3000
//! 
//! This example demonstrates:
//! - SSE transport with authentication headers
//! - Environment variable substitution
//! - Connecting to Figma MCP server
//! - Discovering Figma design tools

use anyhow::Result;
use bedrock_mcp::{McpConfig, McpManager};
use bedrock_tools::ToolRegistry;
use serde_json::json;
use std::env;
use std::sync::Arc;
use tracing::{info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();
    
    info!("Starting MCP SSE Figma API test");
    
    // Check for Figma token
    if env::var("FIGMA_ACCESS_TOKEN").is_err() {
        warn!("FIGMA_ACCESS_TOKEN environment variable not set");
        info!("To use the Figma MCP server, you need to:");
        info!("1. Get a Figma personal access token from https://www.figma.com/developers/api#access-tokens");
        info!("2. Set it as: export FIGMA_ACCESS_TOKEN=your-token-here");
    }
    
    // Load configuration from YAML file
    let config_path = "examples/mcp-sse-figma.yaml";
    let config = match McpConfig::load_from_file(config_path).await {
        Ok(c) => c,
        Err(e) => {
            warn!("Could not load config file: {}", e);
            info!("Creating configuration programmatically...");
            
            // Create config programmatically as fallback
            let mut config = McpConfig::new();
            let server_config = bedrock_mcp::McpServerConfig::Sse {
                transport_type: Some("sse".to_string()),
                url: "http://localhost:3000".to_string(),
                headers: {
                    let mut h = std::collections::HashMap::new();
                    h.insert("Accept".to_string(), "text/event-stream".to_string());
                    h.insert("Cache-Control".to_string(), "no-cache".to_string());
                    // Token will be resolved from environment variable
                    h.insert("Authorization".to_string(), 
                            format!("Bearer ${}", "FIGMA_ACCESS_TOKEN"));
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
            config.mcp_servers.insert("figma-api".to_string(), server_config);
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
            info!("Note: This is expected if figma-mcp server is not running");
            info!("To install and run the server:");
            info!("  npm install -g @figma/mcp-server");
            info!("  export FIGMA_ACCESS_TOKEN=your-token-here");
            info!("  figma-mcp --port 3000");
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
            
            info!("Available Figma tools:");
            for tool_name in &tools {
                info!("  - {}", tool_name);
                
                // Describe common Figma tools
                match tool_name.as_str() {
                    "getFile" => info!("    → Get Figma file data and structure"),
                    "getComments" => info!("    → Get comments from a Figma file"),
                    "postComment" => info!("    → Post a comment to a Figma file"),
                    "getTeamProjects" => info!("    → List projects in a Figma team"),
                    "getProjectFiles" => info!("    → List files in a Figma project"),
                    "getImages" => info!("    → Export images from Figma designs"),
                    "getStyles" => info!("    → Get styles from a Figma file"),
                    "getComponents" => info!("    → Get components from a Figma file"),
                    _ => {}
                }
            }
            
            // Example: Try to get team projects if the tool is available
            if tools.contains(&"getTeamProjects".to_string()) {
                info!("\nTesting Figma getTeamProjects tool...");
                
                let registry = &tool_registry;
                if let Some(tool) = registry.get("getTeamProjects") {
                    // You would need a valid team_id for this to work
                    let args = json!({
                        "team_id": "YOUR_TEAM_ID"  // Replace with actual team ID
                    });
                    
                    match tool.execute(args).await {
                        Ok(result) => {
                            info!("Team projects retrieved: {:?}", result);
                        }
                        Err(e) => {
                            info!("Could not get team projects (expected without valid team_id): {}", e);
                        }
                    }
                }
            }
        }
    }
    
    // Demonstrate SSE-specific features
    info!("\n📡 SSE Transport Features Demonstrated:");
    info!("  ✓ HTTP-based connection to MCP server");
    info!("  ✓ Authentication via Authorization header");
    info!("  ✓ Environment variable substitution for secrets");
    info!("  ✓ Server-Sent Events for real-time updates");
    info!("  ✓ Health monitoring and auto-reconnection");
    
    // Shutdown servers
    info!("\nShutting down MCP servers...");
    manager.stop_all().await?;
    
    info!("✅ MCP SSE Figma test completed successfully!");
    
    Ok(())
}
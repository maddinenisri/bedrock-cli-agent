use bedrock_mcp::client::McpClient;
use bedrock_mcp::config::McpServerConfig;
use serde_json::json;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Create MCP server config using the enum variant
    let config = McpServerConfig::Stdio {
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "figma-developer-mcp".to_string(),
            "--stdio".to_string(),
        ],
        env: HashMap::from([(
            "FIGMA_API_KEY".to_string(),
            std::env::var("FIGMA_API_KEY").unwrap_or_else(|_| "your-figma-api-key".to_string()),
        )]),
        timeout: 30000,
        disabled: false,
        health_check: None,
        restart_policy: None,
    };

    // Create and initialize client
    let mut client = McpClient::new("figma".to_string(), config).await?;
    println!("Initializing MCP client...");
    let init_result = client.initialize().await?;
    println!("Initialized: protocol version {}", init_result.protocol_version);
    if let Some(server_info) = &init_result.server_info {
        println!("Server: {} v{}", server_info.name, server_info.version);
    }

    // List tools
    println!("\nListing tools...");
    let tools = client.list_tools().await?;
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    // Call get_figma_data tool
    println!("\nCalling get_figma_data tool...");
    let args = json!({
        "fileKey": "9LpP8WRtOgcBTL0UfsPthz",
        "nodeId": "138-11634"
    });
    
    let result = client.call_tool("get_figma_data", args).await?;
    println!("\nTool execution successful!");
    
    // Print first part of result (it can be large)
    for (i, content) in result.iter().take(1).enumerate() {
        match content {
            bedrock_mcp::types::ContentItem::Text { text } => {
                let preview = if text.len() > 500 {
                    format!("{}...", &text[..500])
                } else {
                    text.clone()
                };
                println!("Result {}: {}", i + 1, preview);
            }
            bedrock_mcp::types::ContentItem::Image { data, mime_type } => {
                println!("Result {}: Image ({}), {} bytes", i + 1, mime_type, data.len());
            }
        }
    }

    // Close client
    client.close().await?;
    println!("\nTest completed successfully!");

    Ok(())
}
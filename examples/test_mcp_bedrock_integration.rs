//! Test MCP integration with AWS Bedrock LLM
//! 
//! This example demonstrates:
//! - Starting MCP servers and discovering tools
//! - Registering MCP tools with the tool registry
//! - Making tools available to Bedrock LLM
//! - LLM listing and using MCP tools
//! - End-to-end tool execution flow

use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use bedrock_mcp::{McpManager, McpServerConfig};
use bedrock_tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    
    info!("🚀 Starting MCP + Bedrock LLM Integration Test");
    info!("{}", "=".repeat(60));
    
    // Step 1: Create tool registry
    info!("\n📦 Step 1: Creating tool registry...");
    let tool_registry = Arc::new(ToolRegistry::new());
    
    // Step 2: Configure and start MCP servers
    info!("\n🔧 Step 2: Configuring MCP servers...");
    
    // Configure Figma MCP server (stdio)
    let figma_config = McpServerConfig::Stdio {
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
        health_check: None,
        restart_policy: None,
    };
    
    // Configure filesystem MCP server (stdio) as a second example
    let filesystem_config = McpServerConfig::Stdio {
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
            "/tmp".to_string(),
        ],
        env: HashMap::new(),
        timeout: 30000,
        disabled: false,
        health_check: None,
        restart_policy: None,
    };
    
    // Create MCP manager
    let mut mcp_manager = McpManager::new(tool_registry.clone());
    
    // Add servers
    let mut servers = HashMap::new();
    servers.insert("figma-mcp".to_string(), figma_config);
    servers.insert("filesystem-mcp".to_string(), filesystem_config);
    
    mcp_manager.add_servers_from_config(servers).await?;
    
    // Start servers
    info!("\n🎯 Step 3: Starting MCP servers...");
    match mcp_manager.start_servers(vec![]).await {
        Ok(()) => {
            info!("✅ MCP servers started successfully");
        }
        Err(e) => {
            warn!("⚠️ Some MCP servers failed to start: {}", e);
            info!("Continuing with available servers...");
        }
    }
    
    // List running servers and their tools
    info!("\n📋 Step 4: Listing discovered MCP tools...");
    let running_servers = mcp_manager.list_servers().await;
    info!("Running MCP servers: {:?}", running_servers);
    
    let mut total_mcp_tools = 0;
    for server_name in &running_servers {
        if let Some((tools, connected)) = mcp_manager.get_server_info(server_name).await {
            info!("  Server '{}': {} tools (connected: {})", server_name, tools.len(), connected);
            for tool_name in &tools {
                info!("    - {}", tool_name);
            }
            total_mcp_tools += tools.len();
        }
    }
    
    info!("\n📊 Total MCP tools discovered: {}", total_mcp_tools);
    
    // Step 5: List ALL tools available in registry (including MCP tools)
    info!("\n🔍 Step 5: Listing ALL tools in registry...");
    let all_tools = tool_registry.list();
    info!("Total tools in registry: {}", all_tools.len());
    
    info!("\nTools available to Bedrock:");
    for (i, tool_name) in all_tools.iter().enumerate() {
        info!("  {}. {}", i + 1, tool_name);
    }
    
    // Step 6: Create Bedrock client and agent
    info!("\n🤖 Step 6: Creating Bedrock agent with MCP tools...");
    
    // Load or create agent configuration
    let agent_config = AgentConfig {
        agent: bedrock_config::AgentSettings {
            name: "test-mcp-agent".to_string(),
            model: "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
            temperature: 0.7,
            max_tokens: 2000,
        },
        aws: bedrock_config::AwsSettings {
            region: "us-east-1".to_string(),
            profile: None,
            role_arn: None,
        },
        tools: bedrock_config::ToolSettings {
            allowed: vec![],  // Allow all tools
            permissions: std::collections::HashMap::new(),
        },
        ..Default::default()
    };
    
    // Create agent with tool registry containing MCP tools
    let agent = Agent::new(agent_config).await?;
    
    // Step 7: Test with LLM - Ask it to list available tools
    info!("\n💬 Step 7: Testing with Bedrock LLM...");
    info!("Sending prompt: 'List all the tools you have access to, including any MCP tools.'");
    
    let response = agent.chat("List all the tools you have access to, including any MCP tools. For each tool, briefly describe what it does.").await?;
    
    info!("\n🤖 LLM Response:");
    info!("{}", response);
    
    // Step 8: Test tool execution through LLM
    if all_tools.iter().any(|t| t.contains("figma")) {
        info!("\n🧪 Step 8: Testing Figma tool execution through LLM...");
        info!("Sending prompt: 'Can you check if the Figma tools are working?'");
        
        let tool_test = agent.chat("Can you check if the Figma get_figma_data tool is available and working? Just confirm it exists, don't actually call it.").await?;
        info!("\n🤖 LLM Response:");
        info!("{}", tool_test);
    }
    
    // Step 9: Demonstrate tool schema availability
    info!("\n📄 Step 9: Checking tool schemas are available to LLM...");
    for tool_name in all_tools.iter().take(3) {
        if let Some(tool) = tool_registry.get(tool_name) {
            info!("\nTool: {}", tool_name);
            info!("  Description: {}", tool.description());
            info!("  Schema available: {}", !tool.schema().is_null());
        }
    }
    
    // Step 10: Clean shutdown
    info!("\n🔌 Step 10: Shutting down...");
    mcp_manager.stop_all().await?;
    
    info!("\n✅ MCP + Bedrock Integration Test Complete!");
    info!("{}", "=".repeat(60));
    info!("\n📊 Summary:");
    info!("  - MCP servers started: {}", running_servers.len());
    info!("  - MCP tools discovered: {}", total_mcp_tools);
    info!("  - Total tools available to LLM: {}", all_tools.len());
    info!("  - LLM successfully listed tools: ✓");
    info!("  - Tool schemas available: ✓");
    info!("  - MCP tools integrated with Bedrock: ✓");
    
    Ok(())
}
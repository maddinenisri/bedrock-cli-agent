//! Example demonstrating MCP (Model Context Protocol) integration
//! 
//! This example shows how to:
//! 1. Configure MCP servers in the agent config
//! 2. Start an agent with MCP tools
//! 3. Use MCP tools in conversations

use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("bedrock_mcp=debug".parse()?)
        )
        .init();

    println!("🚀 Bedrock Agent with MCP Integration Demo");
    println!("==========================================\n");

    // Load configuration with MCP servers
    let config_path = if std::path::Path::new("examples/mcp-test-simple.yaml").exists() {
        "examples/mcp-test-simple.yaml"
    } else if std::path::Path::new("examples/mcp-stdio-test.yaml").exists() {
        "examples/mcp-stdio-test.yaml"
    } else {
        "config.yaml"  // Fallback to default config
    };

    println!("Loading configuration from: {}", config_path);
    let config = AgentConfig::from_yaml(config_path)?;

    // Check if MCP is enabled
    if !config.mcp.enabled {
        println!("⚠️  MCP is not enabled in the configuration");
        println!("    Set mcp.enabled: true to use MCP servers\n");
    } else {
        println!("✅ MCP is enabled");
        println!("   Config files: {:?}", config.mcp.config_files);
        println!("   Servers to start: {:?}", config.mcp.servers);
        println!("   Inline servers: {} defined\n", config.mcp.inline_servers.len());
    }

    // Create agent (this will initialize MCP servers)
    println!("Initializing agent...");
    let agent = Agent::new(config).await?;
    
    // List connected MCP servers
    let mcp_servers = agent.list_mcp_servers().await;
    if !mcp_servers.is_empty() {
        println!("\n✅ Connected MCP servers:");
        for server in &mcp_servers {
            println!("   - {}", server);
        }
    } else {
        println!("\n⚠️  No MCP servers connected");
    }

    // List available tools
    let tool_registry = agent.get_tool_registry();
    let tools = tool_registry.list();
    
    println!("\n📦 Available tools ({} total):", tools.len());
    for (i, tool_name) in tools.iter().enumerate() {
        if i < 10 {  // Show first 10 tools
            if let Some(tool) = tool_registry.get(tool_name) {
                println!("   - {}: {}", tool.name(), tool.description());
            }
        }
    }
    if tools.len() > 10 {
        println!("   ... and {} more", tools.len() - 10);
    }

    // Interactive demo
    println!("\n💬 Interactive Demo");
    println!("Type 'exit' to quit, 'tools' to list tools, or enter a prompt:\n");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            break;
        }

        if input.eq_ignore_ascii_case("tools") {
            println!("\nAvailable tools:");
            for tool_name in &tools {
                if let Some(tool) = tool_registry.get(tool_name) {
                    println!("  - {}: {}", tool.name(), tool.description());
                }
            }
            println!();
            continue;
        }

        if input.is_empty() {
            continue;
        }

        // Execute as a task
        println!("\n🤖 Processing...\n");
        
        match agent.chat(input).await {
            Ok(response) => {
                println!("{}\n", response);
            }
            Err(e) => {
                eprintln!("❌ Error: {}\n", e);
            }
        }
    }

    // Shutdown
    println!("\n👋 Shutting down...");
    
    // Note: In a real application, you'd want to properly shutdown the agent
    // This would stop all MCP servers cleanly
    // agent.shutdown().await?;

    println!("Goodbye!");
    Ok(())
}

// Example prompts to try:
// 
// With filesystem MCP server:
// - "List all files in the current directory"
// - "Create a file called test.txt with 'Hello from MCP!'"
// - "Read the contents of test.txt"
// 
// With GitHub MCP server:
// - "List my recent GitHub repositories"
// - "Show open issues in my project"
// 
// With custom tools:
// - "What tools are available?"
// - "Use the [tool_name] to [action]"
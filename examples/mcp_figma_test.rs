//! Test MCP integration with actual Figma MCP server
//! 
//! This example:
//! 1. Connects to the Figma MCP server
//! 2. Lists available tools
//! 3. Sends a test query to the LLM

use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use bedrock_core::{Task, Agent as AgentTrait};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("bedrock_mcp=info".parse()?)
                .add_directive("bedrock_agent=info".parse()?)
        )
        .init();

    println!("🎨 Figma MCP Server Integration Test");
    println!("=====================================\n");

    // Load configuration with Figma MCP server
    let config_path = "examples/mcp-stdio-test.yaml";
    println!("Loading configuration from: {}", config_path);
    
    let config = AgentConfig::from_yaml(config_path)?;
    
    // Create agent (this will initialize MCP servers)
    println!("Initializing agent with MCP servers...\n");
    let agent = Agent::new(config).await?;
    
    // Wait a moment for MCP servers to fully initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // List connected MCP servers
    let mcp_servers = agent.list_mcp_servers().await;
    println!("✅ Connected MCP servers:");
    for server in &mcp_servers {
        println!("   - {}", server);
    }
    println!();
    
    // List available tools
    let tool_registry = agent.get_tool_registry();
    let tools = tool_registry.list();
    
    println!("📦 Available tools ({} total):", tools.len());
    
    // Separate Figma tools from others
    let mut figma_tools = Vec::new();
    let mut other_tools = Vec::new();
    
    for tool_name in &tools {
        if let Some(tool) = tool_registry.get(tool_name) {
            let description = tool.description();
            if description.to_lowercase().contains("figma") || 
               tool_name.to_lowercase().contains("figma") {
                figma_tools.push((tool_name.clone(), description.to_string()));
            } else {
                other_tools.push((tool_name.clone(), description.to_string()));
            }
        }
    }
    
    // Display Figma tools
    if !figma_tools.is_empty() {
        println!("\n🎨 Figma Tools:");
        for (name, desc) in &figma_tools {
            println!("   - {}: {}", name, desc);
        }
    }
    
    // Display first few other tools
    if !other_tools.is_empty() {
        println!("\n🔧 Other Tools (first 5):");
        for (name, desc) in other_tools.iter().take(5) {
            println!("   - {}: {}", name, desc);
        }
        if other_tools.len() > 5 {
            println!("   ... and {} more", other_tools.len() - 5);
        }
    }
    
    // Test query to validate with LLM
    println!("\n💬 Testing with LLM query...");
    println!("Query: 'What Figma tools do you have?'\n");
    
    let task = Task::new(
        "What Figma tools do you have? List them with descriptions."
    );
    
    match agent.execute_task(task).await {
        Ok(response) => {
            println!("🤖 Agent Response:");
            println!("{}", response.summary);
            
            // Show metrics if available
            println!("\n📊 Metrics:");
            println!("   - Tokens: {} input, {} output", 
                response.token_stats.input_tokens, response.token_stats.output_tokens);
            println!("   - Cost: ${:.4}", response.cost.total_cost);
        }
        Err(e) => {
            eprintln!("❌ Error executing task: {}", e);
            eprintln!("   This might be due to AWS credentials or model access.");
        }
    }
    
    println!("\n✅ MCP Integration Test Complete!");
    
    // Cleanup
    drop(agent);
    
    Ok(())
}
use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use bedrock_core::{Agent as AgentTrait, Task};
use std::env;
use std::path::PathBuf;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    // Get workspace directory from environment or use current directory
    let workspace_dir = env::var("WORKSPACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_dir().unwrap().join("workspace"));

    // Create workspace if it doesn't exist
    tokio::fs::create_dir_all(&workspace_dir).await?;
    
    println!("🚀 Bedrock CLI Agent Demo");
    println!("========================");
    println!("Workspace: {}", workspace_dir.display());
    println!();

    // Build agent from config file or use default
    let agent = match AgentConfig::from_yaml("examples/agent.yaml") {
        Ok(config) => Agent::new(config).await?,
        Err(_) => {
            println!("Using default configuration");
            Agent::new(AgentConfig::default()).await?
        }
    };

    // Tools are automatically registered in Agent::new with default tools
    println!("✅ Available tools loaded from configuration");
    println!();

    // Get command from arguments or use default
    let args: Vec<String> = env::args().collect();
    let prompt = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        // Default demo prompt
        "Create a file called hello.txt with 'Hello from Bedrock Agent!' and then read it back".to_string()
    };

    println!("📝 Task: {}", prompt);
    println!("Processing...\n");

    // Create and execute task
    let task = Task::new(prompt)
        .with_context("You are a helpful AI assistant with access to file system tools. Use the available tools to complete the task.");

    match agent.execute_task(task).await {
        Ok(result) => {
            println!("✨ Task Completed!");
            println!("================");
            println!("Task ID: {}", result.task_id);
            println!("Status: {:?}", result.status);
            
            println!("\n📄 Response:");
            println!("{}", result.summary);
            
            println!("\n📊 Token Usage:");
            println!("   Input:  {} tokens", result.token_stats.input_tokens);
            println!("   Output: {} tokens", result.token_stats.output_tokens);
            println!("   Total:  {} tokens", result.token_stats.total_tokens);
            
            println!("\n💰 Cost:");
            println!("   Input:  ${:.4}", result.cost.input_cost);
            println!("   Output: ${:.4}", result.cost.output_cost);
            println!("   Total:  ${:.4} {}", result.cost.total_cost, result.cost.currency);
            
            if let Some(error) = result.error {
                println!("\n⚠️ Error: {}", error);
            }
        }
        Err(e) => {
            eprintln!("❌ Task failed: {}", e);
            return Err(e.into());
        }
    }

    println!("\n✅ Task execution complete");
    
    Ok(())
}
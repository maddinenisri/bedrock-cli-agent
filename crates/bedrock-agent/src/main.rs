use anyhow::Result;
use bedrock_agent::{Agent, AgentBuilder};
use bedrock_core::{Agent as AgentTrait, Task};
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    initialize_logging();
    
    info!("Starting Bedrock CLI Agent");
    
    let args: Vec<String> = env::args().collect();
    
    let mut agent = if args.len() > 1 {
        AgentBuilder::new()
            .with_config_file(&args[1])
            .build()
            .await?
    } else {
        Agent::from_default_config().await?
    };
    
    if args.len() > 2 {
        let prompt = args[2..].join(" ");
        let task = Task::new(prompt);
        
        info!("Executing task: {}", task.task_id);
        
        match agent.execute_task(task).await {
            Ok(result) => {
                println!("\n=== Task Result ===");
                println!("Task ID: {}", result.task_id);
                println!("Status: {:?}", result.status);
                println!("Summary: {}", result.summary);
                println!("\nTokens Used:");
                println!("  Input: {}", result.token_stats.input_tokens);
                println!("  Output: {}", result.token_stats.output_tokens);
                println!("  Total: {}", result.token_stats.total_tokens);
                println!("\nCost:");
                println!("  Input: ${:.4}", result.cost.input_cost);
                println!("  Output: ${:.4}", result.cost.output_cost);
                println!("  Total: ${:.4} {}", result.cost.total_cost, result.cost.currency);
                
                if let Some(error) = result.error {
                    println!("\nError: {}", error);
                }
            }
            Err(e) => {
                error!("Task execution failed: {}", e);
                return Err(e.into());
            }
        }
    } else {
        println!("Usage: {} [config.yaml] <prompt>", args[0]);
        println!("\nExample:");
        println!("  {} agent.yaml \"Write a hello world program in Rust\"", args[0]);
    }
    
    agent.shutdown().await?;
    info!("Agent shutdown complete");
    
    Ok(())
}

fn initialize_logging() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false);
    
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
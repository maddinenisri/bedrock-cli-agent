use bedrock_agent::AgentBuilder;
use bedrock_core::{Agent as AgentTrait, Task};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Build agent from config file
    let agent = AgentBuilder::new()
        .with_config_file("examples/agent.yaml")
        .build()
        .await?;

    // Create a task
    let task = Task::new("Write a simple hello world program in Rust")
        .with_context("You are a helpful programming assistant");

    // Execute the task
    let result = agent.execute_task(task).await?;

    // Print results
    println!("Task ID: {}", result.task_id);
    println!("Status: {:?}", result.status);
    println!("\n--- Summary ---");
    println!("{}", result.summary);
    println!("\n--- Token Usage ---");
    println!("Input: {} tokens", result.token_stats.input_tokens);
    println!("Output: {} tokens", result.token_stats.output_tokens);
    println!("Total: {} tokens", result.token_stats.total_tokens);
    println!("\n--- Cost ---");
    println!("Total: ${:.4} {}", result.cost.total_cost, result.cost.currency);

    Ok(())
}
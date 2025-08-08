use bedrock_client::BedrockClient;
use bedrock_config::AgentConfig;
use bedrock_core::{Result, Task};
use bedrock_task::TaskExecutor;
use bedrock_tools::{FileListTool, FileReadTool, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = AgentConfig::from_yaml("config.yaml")
        .unwrap_or_else(|_| AgentConfig::default());

    // Create Bedrock client
    let bedrock_client = Arc::new(BedrockClient::new(config.clone()).await?);

    // Setup tool registry
    let tool_registry = ToolRegistry::new();
    let workspace_dir = PathBuf::from(&config.paths.workspace_dir);
    
    // Register tools
    tool_registry.register(FileReadTool::new(&workspace_dir))?;
    tool_registry.register(FileListTool::new(&workspace_dir))?;

    // Create task executor
    let executor = TaskExecutor::new(
        bedrock_client,
        Arc::new(tool_registry),
        Arc::new(config),
    )?;

    // Create a task
    let mut task = Task::new("List the files in the current directory and summarize what this project is about");
    task.context = "You are a helpful assistant. Use the available tools to explore the project structure.".to_string();

    // Execute the task
    println!("Executing task: {}", task.prompt);
    println!("Please wait...\n");

    let result = executor.execute_task(task).await?;

    // Display results
    println!("Task ID: {}", result.task_id);
    println!("Status: {:?}", result.status);
    println!("\nSummary:");
    println!("{}", result.summary);
    println!("\nTokens used: {}", result.token_stats.total_tokens);
    println!("Cost: ${:.4} USD", result.cost.total_cost);

    if let Some(error) = result.error {
        println!("\nError: {}", error);
    }

    Ok(())
}
use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use bedrock_conversation::{ConversationManager, ConversationStorage, MessageEntry, ConversationMetadata};
use bedrock_core::{Agent as AgentTrait, Task, TaskStatus, TaskResult};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "bedrock-agent")]
#[command(about = "AWS Bedrock LLM Agent with built-in tools", long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
    config: PathBuf,

    #[arg(short, long, action = clap::ArgAction::SetTrue)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage conversations (resume, summary, export, delete)
    Conversation {
        /// The conversation ID
        #[arg(value_name = "ID")]
        id: String,
        
        /// Resume the conversation (default action)
        #[arg(long, action = clap::ArgAction::SetTrue)]
        resume: bool,
        
        /// Generate an AI summary
        #[arg(long, action = clap::ArgAction::SetTrue)]
        summary: bool,
        
        /// Export to JSON file
        #[arg(long, value_name = "FILE")]
        export: Option<PathBuf>,
        
        /// Delete the conversation
        #[arg(long, action = clap::ArgAction::SetTrue)]
        delete: bool,
        
        /// Skip confirmation for delete
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        force: bool,
        
        /// Use streaming mode (for resume)
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        stream: bool,
    },

    /// Execute or manage tasks
    Task {
        /// Task ID to resume or prompt to execute
        #[arg(value_name = "ID_OR_PROMPT")]
        input: String,
        
        /// Resume a task by ID (auto-detected if UUID format)
        #[arg(long, action = clap::ArgAction::SetTrue)]
        resume: bool,
        
        /// Additional prompt (for resume or new task)
        #[arg(short, long)]
        prompt: Option<String>,
        
        /// Context for new task
        #[arg(short, long)]
        context: Option<String>,
        
        /// Export task to file
        #[arg(long, value_name = "FILE")]
        export: Option<PathBuf>,
        
        /// Use streaming mode
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        stream: bool,
    },

    /// Import conversations or tasks from JSON
    Import {
        /// File to import from
        #[arg(value_name = "FILE")]
        file: PathBuf,
        
        /// Type of import (auto-detected if not specified)
        #[arg(long, value_enum)]
        import_type: Option<ImportType>,
        
        /// Resume after importing
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        resume: bool,
        
        /// Force overwrite existing
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        force: bool,
        
        /// Use streaming mode (for resume)
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        stream: bool,
    },

    /// List conversations, tasks, or show statistics
    List {
        /// What to list
        #[arg(long, value_enum, default_value = "conversations")]
        list_type: ListType,
        
        /// Show statistics
        #[arg(long, action = clap::ArgAction::SetTrue)]
        stats: bool,
        
        /// Show only tasks (shorthand)
        #[arg(long, action = clap::ArgAction::SetTrue)]
        tasks: bool,
        
        /// Verbose output
        #[arg(long, action = clap::ArgAction::SetTrue)]
        verbose: bool,
    },

    /// Interactive conversation mode
    Chat {
        /// Initial system prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Use streaming mode
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        stream: bool,
    },

    /// List available tools
    Tools,

    /// Test AWS credentials and connectivity
    Test,
}

#[derive(clap::ValueEnum, Clone)]
enum ImportType {
    Conversation,
    Task,
}

#[derive(clap::ValueEnum, Clone)]
enum ListType {
    Conversations,
    Tasks,
    All,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose)?;

    // Load configuration
    let config = if cli.config.exists() {
        info!("Loading configuration from: {:?}", cli.config);
        AgentConfig::from_yaml(&cli.config)?
    } else {
        info!("Using default configuration");
        AgentConfig::default()
    };

    // Create agent
    let agent = Agent::new(config).await?;

    match cli.command {
        Commands::Conversation { id, resume, summary, export, delete, force, stream } => {
            handle_conversation_command(agent, id, resume, summary, export, delete, force, stream).await?;
        }
        Commands::Task { input, resume, prompt, context, export, stream } => {
            handle_task_command(agent, input, resume, prompt, context, export, stream).await?;
        }
        Commands::Import { file, import_type, resume, force, stream } => {
            handle_import_command(agent, file, import_type, resume, force, stream).await?;
        }
        Commands::List { list_type, stats, tasks, verbose } => {
            handle_list_command(list_type, stats, tasks, verbose).await?;
        }
        Commands::Chat { system, stream } => {
            interactive_chat(agent, system, stream).await?;
        }
        Commands::Tools => {
            list_tools(&agent);
        }
        Commands::Test => {
            test_connectivity(&agent).await?;
        }
    }

    Ok(())
}

async fn execute_task(
    agent: Agent,
    prompt: String,
    context: Option<String>,
    stream: bool,
) -> Result<()> {
    info!("Executing task: {}", prompt);
    
    if stream {
        println!("\n🤖 Streaming response:\n");
        
        let result = agent.chat_stream(&prompt, |chunk| {
            print!("{chunk}");
            std::io::stdout().flush().ok();
        }).await?;
        
        println!("\n");
        
        // Display metrics after streaming
        println!("\n📊 Token Statistics:");
        println!("  Input tokens: {}", result.token_stats.input_tokens);
        println!("  Output tokens: {}", result.token_stats.output_tokens);
        println!("  Total tokens: {}", result.token_stats.total_tokens);
        
        println!("\n💰 Cost Details:");
        println!("  Model: {}", result.cost.model);
        println!("  Input cost: ${:.4}", result.cost.input_cost);
        println!("  Output cost: ${:.4}", result.cost.output_cost);
        println!("  Total cost: ${:.4} {}", result.cost.total_cost, result.cost.currency);
    } else {
        // For non-streaming, use the task execution for full tracking
        let task = if let Some(ctx) = context {
            Task::new(&prompt).with_context(ctx)
        } else {
            Task::new(&prompt)
        };
        
        let result = agent.execute_task(task).await?;
        
        println!("\n📋 Task Result");
        println!("═══════════════════════════════════════");
        println!("Task ID: {}", result.task_id);
        println!("Status: {:?}", result.status);
        println!("Summary: {}", result.summary);
        
        if result.status == TaskStatus::Failed {
            if let Some(error) = &result.error {
                println!("Error: {error}");
            }
        }
        
        println!("\n💬 Conversation:");
        if let Some(conversation) = &result.conversation {
            for msg in conversation {
                if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                        // Format based on role and content
                        match role {
                            "User" => {
                                if content.starts_with("Tool result") {
                                    println!("📊 {}", content);
                                } else {
                                    println!("👤 User: {}", content);
                                }
                            },
                            "Assistant" => {
                                if content.contains("Using tool:") {
                                    // Split content by tool uses for better formatting
                                    for line in content.lines() {
                                        if line.starts_with("Using tool:") {
                                            println!("🔧 {}", line);
                                        } else {
                                            println!("🤖 Assistant: {}", line);
                                        }
                                    }
                                } else {
                                    println!("🤖 Assistant: {}", content);
                                }
                            },
                            _ => {
                                println!("[{role}]: {content}");
                            }
                        }
                        println!();
                    }
                }
            }
        }
        
        println!("\n📊 Token Statistics:");
        println!("  Input tokens: {}", result.token_stats.input_tokens);
        println!("  Output tokens: {}", result.token_stats.output_tokens);
        println!("  Total tokens: {}", result.token_stats.total_tokens);
        
        println!("\n💰 Cost Details:");
        println!("  Model: {}", result.cost.model);
        println!("  Input cost: ${:.4}", result.cost.input_cost);
        println!("  Output cost: ${:.4}", result.cost.output_cost);
        println!("  Total cost: ${:.4} {}", result.cost.total_cost, result.cost.currency);
    }
    
    Ok(())
}

async fn interactive_chat(
    agent: Agent,
    _system_prompt: Option<String>,
    stream: bool,
) -> Result<()> {
    
    println!("🤖 Bedrock Agent Interactive Chat");
    println!("Type 'exit' or 'quit' to end the conversation");
    println!("Type 'tools' to see available tools");
    println!("═══════════════════════════════════════\n");
    
    loop {
        print!("You> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("Goodbye!");
            break;
        }
        
        if input.eq_ignore_ascii_case("tools") {
            list_tools(&agent);
            continue;
        }
        
        print!("\nAssistant> ");
        io::stdout().flush()?;
        
        if stream {
            let result = agent.chat_stream(input, |chunk| {
                print!("{chunk}");
                std::io::stdout().flush().ok();
            }).await?;
            println!("\n");
            // Optionally show metrics in chat mode too (in a more compact format)
            println!("(Tokens: {} | Cost: ${:.4})", 
                result.token_stats.total_tokens, 
                result.cost.total_cost);
        } else {
            let response = agent.chat(input).await?;
            println!("{response}\n");
        }
    }
    
    Ok(())
}

fn list_tools(agent: &Agent) {
    println!("\n🛠️  Available Tools:");
    println!("═══════════════════════════════════════");
    
    let tool_registry = agent.get_tool_registry();
    for tool_name in tool_registry.list() {
        if let Some(tool) = tool_registry.get(&tool_name) {
            println!("\n📦 {}", tool.name());
            println!("   {}", tool.description());
        }
    }
    println!();
}

async fn test_connectivity(agent: &Agent) -> Result<()> {
    println!("\n🔍 Testing AWS Bedrock Connectivity");
    println!("═══════════════════════════════════════");
    
    print!("\nTesting API connection... ");
    io::stdout().flush()?;
    
    let test_task = Task::new("Hello, can you hear me?");
    match agent.execute_task(test_task).await {
        Ok(result) => {
            if result.status == TaskStatus::Completed {
                println!("✅ Success!");
                println!("Response: {}", result.summary);
                println!("\nToken usage: {} tokens", result.token_stats.total_tokens);
                println!("Estimated cost: ${:.4}", result.cost.total_cost);
            } else {
                println!("❌ Failed");
                println!("Error: {:?}", result.error);
            }
        }
        Err(e) => {
            println!("❌ Failed");
            println!("Error: {e}");
        }
    }
    
    Ok(())
}

// Unified command handlers

async fn handle_conversation_command(
    agent: Agent,
    id: String,
    _resume: bool,
    summary: bool,
    export: Option<PathBuf>,
    delete: bool,
    force: bool,
    stream: bool,
) -> Result<()> {
    // Parse the conversation ID
    let _conv_id = Uuid::parse_str(&id)
        .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?;
    
    // Handle different operations
    if delete {
        delete_conversation(id, force).await?;
    } else if summary {
        generate_conversation_summary(agent, id).await?;
    } else if let Some(export_path) = export {
        export_conversation(id, Some(export_path)).await?;
    } else {
        // Default action is resume
        resume_conversation(agent, id, stream).await?;
    }
    
    Ok(())
}

async fn handle_task_command(
    agent: Agent,
    input: String,
    resume: bool,
    prompt: Option<String>,
    context: Option<String>,
    export: Option<PathBuf>,
    stream: bool,
) -> Result<()> {
    // Check if input is a UUID (task ID) or a prompt
    let is_uuid = Uuid::parse_str(&input).is_ok();
    
    if is_uuid || resume {
        // Resume existing task
        if let Some(export_path) = export {
            // Export task
            export_task(input.clone(), export_path).await?;
        } else {
            // Resume task with optional prompt
            resume_task(agent, input, prompt, stream).await?;
        }
    } else {
        // Execute new task
        let task_prompt = prompt.unwrap_or(input);
        execute_task(agent, task_prompt, context, stream).await?;
    }
    
    Ok(())
}

async fn handle_import_command(
    agent: Agent,
    file: PathBuf,
    import_type: Option<ImportType>,
    resume: bool,
    force: bool,
    stream: bool,
) -> Result<()> {
    // Auto-detect type if not specified
    let detected_type = if let Some(t) = import_type {
        t
    } else {
        detect_import_type(&file).await?
    };
    
    match detected_type {
        ImportType::Conversation => {
            import_conversation(file, force).await?;
        }
        ImportType::Task => {
            import_task(agent, file, resume, stream).await?;
        }
    }
    
    Ok(())
}

async fn handle_list_command(
    list_type: ListType,
    stats: bool,
    tasks: bool,
    verbose: bool,
) -> Result<()> {
    // Override list_type if tasks flag is set
    let actual_type = if tasks {
        ListType::Tasks
    } else {
        list_type
    };
    
    if stats {
        show_conversation_stats().await?;
    } else {
        match actual_type {
            ListType::Conversations => list_conversations().await?,
            ListType::Tasks => list_tasks(verbose).await?,
            ListType::All => {
                list_conversations().await?;
                println!(); // Separator
                list_tasks(verbose).await?;
            }
        }
    }
    
    Ok(())
}

// Helper function to detect import type
async fn detect_import_type(file: &PathBuf) -> Result<ImportType> {
    let content = fs::read_to_string(file)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;
    
    // Check for task-specific fields
    if json.get("task_id").is_some() || json.get("status").is_some() {
        Ok(ImportType::Task)
    } else if json.get("conversation_id").is_some() || json.get("messages").is_some() {
        Ok(ImportType::Conversation)
    } else {
        Err(anyhow::anyhow!("Cannot auto-detect import type. Please specify --import-type"))
    }
}

// Export task function
async fn export_task(task_id: String, output: PathBuf) -> Result<()> {
    // Parse the task ID
    let _task_uuid = Uuid::parse_str(&task_id)
        .map_err(|e| anyhow::anyhow!("Invalid task ID: {}", e))?;
    
    // Find the task in conversations
    let storage = ConversationStorage::new()?;
    let conversations = storage.list_conversations()?;
    
    for conv_summary in conversations {
        if conv_summary.has_tasks {
            let messages = storage.read_messages(&conv_summary.id)?;
            
            // Look for task results in messages
            for msg in &messages {
                if msg.role == "assistant" {
                    if let Some(text) = msg.content.as_str() {
                        if text.contains(&task_id) {
                            // Create task export
                            let export = serde_json::json!({
                                "task_id": task_id,
                                "conversation_id": conv_summary.id,
                                "created_at": conv_summary.created_at,
                                "messages": messages,
                            });
                            
                            let json_str = serde_json::to_string_pretty(&export)?;
                            fs::write(&output, json_str)?;
                            println!("✅ Exported task to: {}", output.display());
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("Task ID not found: {}", task_id))
}

// List tasks function
async fn list_tasks(verbose: bool) -> Result<()> {
    let storage = ConversationStorage::new()?;
    let conversations = storage.list_conversations()?;
    
    let mut task_count = 0;
    println!("\n📋 Tasks in current workspace:\n");
    
    if verbose {
        println!("{:<38} {:<38} {:<20} {:<10}", "Task ID", "Conversation ID", "Created", "Status");
        println!("{}", "-".repeat(106));
    }
    
    for conv in conversations {
        if conv.has_tasks {
            let messages = storage.read_messages(&conv.id)?;
            
            for msg in messages {
                if msg.role == "assistant" {
                    if let Some(text) = msg.content.as_str() {
                        // Extract task IDs from messages
                        if text.contains("Task ID:") {
                            if let Some(start) = text.find("Task ID:") {
                                let id_start = start + 9;
                                if let Some(end) = text[id_start..].find('\n') {
                                    let task_id = &text[id_start..id_start + end].trim();
                                    
                                    if verbose {
                                        let status = if text.contains("Status: Completed") {
                                            "✅"
                                        } else if text.contains("Status: Failed") {
                                            "❌"
                                        } else {
                                            "⏳"
                                        };
                                        
                                        println!(
                                            "{:<38} {:<38} {:<20} {:<10}",
                                            task_id,
                                            conv.id,
                                            conv.created_at.format("%Y-%m-%d %H:%M"),
                                            status
                                        );
                                    } else {
                                        println!("  {}", task_id);
                                    }
                                    task_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    if task_count == 0 {
        println!("No tasks found in the current workspace.");
    } else {
        println!("\nTotal tasks: {}", task_count);
        println!("Use 'bedrock-agent task <id> --resume' to continue a task");
    }
    
    Ok(())
}

async fn list_conversations() -> Result<()> {
    let storage = ConversationStorage::new()?;
    let conversations = storage.list_conversations()?;
    
    if conversations.is_empty() {
        println!("No conversations found in the current workspace.");
        return Ok(());
    }
    
    println!("\n📚 Conversations in current workspace:\n");
    println!("{:<38} {:<20} {:<10} {:<10} {:<10}", "ID", "Updated", "Messages", "Tasks", "Status");
    println!("{}", "-".repeat(88));
    
    for conv in conversations {
        let status = if conv.has_tasks {
            format!("✓{}/✗{}", conv.completed_tasks, conv.failed_tasks)
        } else {
            "-".to_string()
        };
        
        println!(
            "{:<38} {:<20} {:<10} {:<10} {:<10}",
            conv.id,
            conv.updated_at.format("%Y-%m-%d %H:%M"),
            conv.message_count,
            conv.task_count,
            status
        );
    }
    
    println!("\nUse 'bedrock-agent resume <id>' to continue a conversation");
    Ok(())
}

async fn resume_conversation(agent: Agent, conversation_id: String, stream: bool) -> Result<()> {
    // Parse the conversation ID
    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?;
    
    // Load the conversation
    let mut manager = ConversationManager::new()?;
    let messages = manager.resume_conversation(conv_id)?;
    
    println!("\n📖 Resumed conversation: {}", conv_id);
    println!("Found {} previous messages\n", messages.len());
    
    // Display conversation history
    for (_i, msg) in messages.iter().enumerate() {
        let role_emoji = match msg.role.as_str() {
            "user" => "👤",
            "assistant" => "🤖",
            _ => "🔧",
        };
        
        // Extract text content if available
        let content = if let Some(text) = msg.content.as_str() {
            text.to_string()
        } else if let Some(array) = msg.content.as_array() {
            array.iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(|t| t.as_str())
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            format!("{:?}", msg.content)
        };
        
        if !content.trim().is_empty() {
            println!("{} [{}]: {}", role_emoji, msg.role, 
                if content.len() > 100 {
                    format!("{}...", &content[..97])
                } else {
                    content
                }
            );
        }
    }
    
    println!("\n--- Continuing conversation ---\n");
    
    // Now enter interactive mode with this conversation
    interactive_chat_with_history(agent, conv_id, stream).await
}

async fn export_conversation(conversation_id: String, output: Option<PathBuf>) -> Result<()> {
    // Parse the conversation ID
    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?;
    
    let storage = ConversationStorage::new()?;
    
    // Load metadata and messages
    let metadata = storage.load_metadata(&conv_id)?;
    let messages = storage.read_messages(&conv_id)?;
    
    // Create export JSON
    let export = serde_json::json!({
        "conversation_id": conv_id,
        "model": metadata.model_id,
        "created_at": metadata.created_at,
        "updated_at": metadata.updated_at,
        "message_count": metadata.message_count,
        "token_usage": metadata.token_usage,
        "messages": messages,
    });
    
    let json_str = serde_json::to_string_pretty(&export)?;
    
    if let Some(output_path) = output {
        std::fs::write(&output_path, json_str)?;
        println!("✅ Exported conversation to: {}", output_path.display());
    } else {
        println!("{}", json_str);
    }
    
    Ok(())
}

async fn delete_conversation(conversation_id: String, force: bool) -> Result<()> {
    // Parse the conversation ID
    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?;
    
    let storage = ConversationStorage::new()?;
    
    // Load metadata to show info
    let metadata = storage.load_metadata(&conv_id)?;
    
    println!("Conversation to delete:");
    println!("  ID: {}", conv_id);
    println!("  Created: {}", metadata.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("  Messages: {}", metadata.message_count);
    
    if !force {
        print!("\nAre you sure you want to delete this conversation? (y/N): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Deletion cancelled.");
            return Ok(());
        }
    }
    
    storage.delete_conversation(&conv_id)?;
    println!("✅ Conversation deleted successfully");
    
    Ok(())
}

async fn show_conversation_stats() -> Result<()> {
    let storage = ConversationStorage::new()?;
    let conversations = storage.list_conversations()?;
    
    if conversations.is_empty() {
        println!("No conversations found in the current workspace.");
        return Ok(());
    }
    
    // Calculate statistics
    let total_conversations = conversations.len();
    let total_messages: usize = conversations.iter().map(|c| c.message_count).sum();
    let total_tasks: usize = conversations.iter().map(|c| c.task_count).sum();
    let completed_tasks: usize = conversations.iter().map(|c| c.completed_tasks).sum();
    let failed_tasks: usize = conversations.iter().map(|c| c.failed_tasks).sum();
    
    let oldest = conversations.iter()
        .min_by_key(|c| c.created_at)
        .map(|c| c.created_at);
    
    let newest = conversations.iter()
        .max_by_key(|c| c.updated_at)
        .map(|c| c.updated_at);
    
    // Calculate total token usage
    let mut total_tokens = 0u32;
    let mut total_cost = 0.0f64;
    
    for conv in &conversations {
        if let Ok(metadata) = storage.load_metadata(&conv.id) {
            total_tokens += metadata.token_usage.total_tokens;
            if let Some(cost) = metadata.token_usage.total_cost {
                total_cost += cost;
            }
        }
    }
    
    println!("\n📊 Conversation Statistics\n");
    println!("Workspace: {}", storage.get_workspace_dir().display());
    println!("{}", "-".repeat(50));
    println!("Total Conversations: {}", total_conversations);
    println!("Total Messages:      {}", total_messages);
    println!("Average Messages:    {:.1}", total_messages as f64 / total_conversations as f64);
    println!();
    println!("Total Tasks:         {}", total_tasks);
    println!("Completed Tasks:     {} ({:.1}%)", 
        completed_tasks, 
        if total_tasks > 0 { completed_tasks as f64 / total_tasks as f64 * 100.0 } else { 0.0 }
    );
    println!("Failed Tasks:        {} ({:.1}%)", 
        failed_tasks,
        if total_tasks > 0 { failed_tasks as f64 / total_tasks as f64 * 100.0 } else { 0.0 }
    );
    println!();
    println!("Total Tokens Used:   {}", total_tokens);
    println!("Total Cost:          ${:.4} USD", total_cost);
    
    if let Some(oldest) = oldest {
        println!("\nOldest Conversation: {}", oldest.format("%Y-%m-%d %H:%M:%S"));
    }
    if let Some(newest) = newest {
        println!("Latest Activity:     {}", newest.format("%Y-%m-%d %H:%M:%S"));
    }
    
    Ok(())
}

// Helper function for resuming conversations
async fn interactive_chat_with_history(
    agent: Agent,
    _conversation_id: Uuid,
    stream: bool,
) -> Result<()> {
    println!("Entering interactive mode with resumed conversation. Type 'exit' or 'quit' to stop.");
    println!("Type 'help' for available commands.\n");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }

        if input == "help" {
            println!("\nAvailable commands:");
            println!("  exit/quit - Exit the chat");
            println!("  help      - Show this help message");
            println!("\nJust type your message to continue the conversation.\n");
            continue;
        }

        // Continue conversation with the agent
        if stream {
            println!("\n🤖 Streaming response:\n");
            let callback = |chunk: &str| {
                print!("{}", chunk);
                io::stdout().flush().unwrap();
            };
            
            match agent.chat_stream(input, callback).await {
                Ok(result) => {
                    println!("\n\n📊 Token usage: {} input, {} output", 
                             result.token_stats.input_tokens, 
                             result.token_stats.output_tokens);
                }
                Err(e) => eprintln!("\n❌ Error: {}", e),
            }
        } else {
            print!("\n🤖 Assistant: ");
            io::stdout().flush()?;
            
            match agent.chat(input).await {
                Ok(response) => println!("{}\n", response),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
    }
    
    Ok(())
}

// New conversation management functions

async fn generate_conversation_summary(agent: Agent, conversation_id: String) -> Result<()> {
    // Parse the conversation ID
    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| anyhow::anyhow!("Invalid conversation ID: {}", e))?;
    
    // Load the conversation
    let storage = ConversationStorage::new()?;
    let messages = storage.read_messages(&conv_id)?;
    let metadata = storage.load_metadata(&conv_id)?;
    
    if messages.is_empty() {
        println!("No messages found in conversation {}", conv_id);
        return Ok(());
    }
    
    println!("\n📝 Generating summary for conversation: {}", conv_id);
    println!("Model: {}", metadata.model_id);
    println!("Messages: {}", messages.len());
    println!("Created: {}", metadata.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("\n─────────────────────────────────────");
    
    // Prepare conversation context for summary
    let mut context = String::from("Please provide a concise summary of the following conversation:\n\n");
    
    for msg in &messages {
        let role_str = &msg.role;
        let content_str = if let Some(text) = msg.content.as_str() {
            text.to_string()
        } else if let Some(array) = msg.content.as_array() {
            array.iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(|t| t.as_str())
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            continue;
        };
        
        if !content_str.trim().is_empty() && role_str != "tool" {
            context.push_str(&format!("{}: {}\n", role_str, 
                if content_str.len() > 500 {
                    format!("{}...", &content_str[..497])
                } else {
                    content_str
                }
            ));
        }
    }
    
    context.push_str("\n\nProvide a brief summary highlighting the main topics discussed, decisions made, and any important outcomes or next steps.");
    
    // Generate summary using the agent
    println!("\n🤖 AI-Generated Summary:\n");
    let summary = agent.chat(&context).await?;
    println!("{}\n", summary);
    
    // Show token statistics if available
    println!("─────────────────────────────────────");
    println!("📊 Conversation Statistics:");
    println!("  Total tokens used: {}", metadata.token_usage.total_tokens);
    if let Some(cost) = metadata.token_usage.total_cost {
        println!("  Total cost: ${:.4} USD", cost);
    }
    
    Ok(())
}

async fn resume_task(agent: Agent, task_id: String, prompt: Option<String>, stream: bool) -> Result<()> {
    // Parse the task ID
    let _task_uuid = Uuid::parse_str(&task_id)
        .map_err(|e| anyhow::anyhow!("Invalid task ID: {}", e))?;
    
    // Find the task in conversations
    let storage = ConversationStorage::new()?;
    let conversations = storage.list_conversations()?;
    
    println!("\n🔍 Searching for task: {}", task_id);
    
    // Search through conversations for the task
    for conv_summary in conversations {
        if conv_summary.has_tasks {
            let messages = storage.read_messages(&conv_summary.id)?;
            
            // Look for task results in messages
            for msg in &messages {
                if msg.role == "assistant" {
                    // Check if this message contains our task ID
                    if let Some(text) = msg.content.as_str() {
                        if text.contains(&task_id) {
                            println!("\n✅ Found task in conversation: {}", conv_summary.id);
                            println!("Created: {}", conv_summary.created_at.format("%Y-%m-%d %H:%M:%S"));
                            
                            // Load the full conversation context
                            let mut manager = ConversationManager::new()?;
                            let _history = manager.resume_conversation(conv_summary.id)?;
                            
                            println!("\n📋 Task Context Loaded");
                            println!("─────────────────────────────────────");
                            
                            // Extract task summary from the message
                            if text.contains("Task ID:") && text.contains("Summary:") {
                                let summary_start = text.find("Summary:").unwrap() + 8;
                                let summary_end = text[summary_start..].find('\n')
                                    .map(|i| summary_start + i)
                                    .unwrap_or(text.len());
                                let summary = &text[summary_start..summary_end].trim();
                                println!("Previous task summary: {}", summary);
                            }
                            
                            // Continue with the provided prompt or enter interactive mode
                            if let Some(continue_prompt) = prompt {
                                println!("\n🚀 Continuing task with: {}", continue_prompt);
                                
                                if stream {
                                    println!("\n🤖 Streaming response:\n");
                                    let result = agent.chat_stream(&continue_prompt, |chunk| {
                                        print!("{}", chunk);
                                        std::io::stdout().flush().ok();
                                    }).await?;
                                    println!("\n\n📊 Token usage: {} total", result.token_stats.total_tokens);
                                } else {
                                    let response = agent.chat(&continue_prompt).await?;
                                    println!("\n🤖 Response:\n{}", response);
                                }
                            } else {
                                println!("\nEntering interactive mode to continue the task...");
                                interactive_chat_with_history(agent, conv_summary.id, stream).await?;
                            }
                            
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    
    println!("\n❌ Task ID not found: {}", task_id);
    println!("Please check the task ID or run 'list-conversations' to see available conversations.");
    
    Ok(())
}

async fn import_conversation(file: PathBuf, force: bool) -> Result<()> {
    println!("\n📥 Importing conversation from: {}", file.display());
    
    // Read the JSON file
    let json_content = fs::read_to_string(&file)
        .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;
    
    // Parse the JSON
    let import_data: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON format: {}", e))?;
    
    // Extract conversation ID
    let conv_id = if let Some(id_str) = import_data.get("conversation_id").and_then(|v| v.as_str()) {
        Uuid::parse_str(id_str)?
    } else {
        return Err(anyhow::anyhow!("Missing conversation_id in import file"));
    };
    
    let storage = ConversationStorage::new()?;
    
    // Check if conversation already exists by trying to load metadata
    let exists = storage.load_metadata(&conv_id).is_ok();
    
    if exists && !force {
        print!("\n⚠️  Conversation {} already exists. Overwrite? (y/N): ", conv_id);
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Import cancelled.");
            return Ok(());
        }
    }
    
    // Extract messages
    let messages = import_data.get("messages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid messages array"))?;
    
    println!("Found {} messages to import", messages.len());
    
    // Import messages directly to storage
    let storage = ConversationStorage::new()?;
    
    // Create metadata from import data
    let _metadata = if let Ok(existing_meta) = storage.load_metadata(&conv_id) {
        existing_meta
    } else {
        // Create new metadata
        let model_id = import_data.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic.claude-3-5-sonnet-20241022-v2:0")
            .to_string();
        
        let mut meta = ConversationMetadata::new(model_id, None);
        meta.id = conv_id;
        
        if let Some(created) = import_data.get("created_at").and_then(|v| v.as_str()) {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created) {
                meta.created_at = dt.with_timezone(&Utc);
            }
        }
        
        storage.save_metadata(&meta)?;
        meta
    };
    
    // Import each message
    for (i, msg_value) in messages.iter().enumerate() {
        let msg: MessageEntry = serde_json::from_value(msg_value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse message {}: {}", i, e))?;
        
        // Append message to conversation
        storage.append_message(&conv_id, &msg)?;
    }
    
    println!("✅ Successfully imported conversation: {}", conv_id);
    println!("Use 'bedrock-agent resume {}' to continue this conversation", conv_id);
    
    Ok(())
}

async fn import_task(agent: Agent, file: PathBuf, resume: bool, stream: bool) -> Result<()> {
    println!("\n📥 Importing task from: {}", file.display());
    
    // Read the JSON file
    let json_content = fs::read_to_string(&file)
        .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;
    
    // Parse as TaskResult
    let task_result: TaskResult = serde_json::from_str(&json_content)
        .map_err(|e| anyhow::anyhow!("Invalid task result format: {}", e))?;
    
    println!("\n📋 Task Imported:");
    println!("  Task ID: {}", task_result.task_id);
    println!("  Status: {:?}", task_result.status);
    println!("  Summary: {}", task_result.summary);
    
    if task_result.status == TaskStatus::Failed {
        if let Some(error) = &task_result.error {
            println!("  Error: {}", error);
        }
    }
    
    // Create a new conversation from the task
    let storage = ConversationStorage::new()?;
    let conv_id = Uuid::new_v4();
    
    // Create metadata for the imported task
    let model_id = "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string();
    let mut metadata = ConversationMetadata::new(model_id, None);
    metadata.id = conv_id;
    metadata.has_tasks = true;
    metadata.task_count = 1;
    
    if task_result.status == TaskStatus::Completed {
        metadata.completed_tasks = 1;
    } else if task_result.status == TaskStatus::Failed {
        metadata.failed_tasks = 1;
    }
    
    storage.save_metadata(&metadata)?;
    
    // Add task messages to conversation
    if let Some(conversation) = &task_result.conversation {
        for msg_value in conversation {
            // Convert the JSON value to a MessageEntry
            if let (Some(role), Some(content)) = (msg_value.get("role"), msg_value.get("content")) {
                let msg = if role.as_str() == Some("user") {
                    MessageEntry::user(content.as_str().unwrap_or("").to_string())
                } else if role.as_str() == Some("assistant") {
                    MessageEntry::assistant(content.as_str().unwrap_or("").to_string())
                } else {
                    continue;
                };
                storage.append_message(&conv_id, &msg)?;
            }
        }
    }
    
    println!("\n✅ Task imported as conversation: {}", conv_id);
    
    // Resume if requested
    if resume {
        println!("\n🚀 Resuming imported task...");
        interactive_chat_with_history(agent, conv_id, stream).await?;
    } else {
        println!("Use 'bedrock-agent resume {}' to continue this task", conv_id);
    }
    
    Ok(())
}

fn init_logging(verbose: bool) -> Result<()> {
    let filter = if verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter))
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    Ok(())
}
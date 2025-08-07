use anyhow::Result;
use bedrock_agent::Agent;
use bedrock_config::AgentConfig;
use bedrock_core::{Agent as AgentTrait, Task, TaskStatus};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    /// Execute a single task
    Task {
        /// The prompt to execute
        #[arg(short, long)]
        prompt: String,

        /// Optional context for the task
        #[arg(short, long)]
        context: Option<String>,

        /// Use streaming mode
        #[arg(short, long, action = clap::ArgAction::SetTrue)]
        stream: bool,
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
        Commands::Task { prompt, context, stream } => {
            execute_task(agent, prompt, context, stream).await?;
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
        for msg in &result.conversation {
            if let Some(role) = msg.get("role") {
                if let Some(content) = msg.get("content") {
                    println!("[{role}]: {content}");
                    println!();
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
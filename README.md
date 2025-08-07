# Bedrock CLI Agent

A modular Rust-based agent for interacting with AWS Bedrock LLM with comprehensive tooling capabilities, MCP integration, and cost tracking.

## Features

- 🚀 **AWS Bedrock Integration**: Full support for AWS Bedrock conversation APIs with streaming
- 🔧 **Modular Tool System**: Extensible tool framework with built-in file operations and search
- 🌐 **MCP Protocol Support**: Integrate with MCP servers via stdio and SSE transports
- 💾 **Intelligent Caching**: LRU cache with persistence to reduce API calls
- 📊 **Cost Tracking**: Real-time token usage and cost calculation
- 🔒 **Security First**: Path validation, permission system, and sandboxed operations
- 📈 **Observability**: Comprehensive metrics, structured logging, and tracing

## Architecture

```
bedrock-agent/
├── crates/
│   ├── bedrock-core/     # Core types and traits
│   ├── bedrock-client/   # AWS Bedrock client
│   ├── bedrock-tools/    # Reusable tool system
│   ├── bedrock-mcp/      # MCP integration
│   ├── bedrock-task/     # Task processing engine
│   ├── bedrock-config/   # Configuration management
│   ├── bedrock-metrics/  # Token tracking & costs
│   └── bedrock-agent/    # Main agent orchestration
```

## Quick Start

### Installation

```bash
cargo build --release
```

### Configuration

Create an `agent.yaml` file in your `$HOME_DIR`:

```yaml
agent:
  name: "my-bedrock-agent"
  model: "anthropic.claude-3-sonnet"
  
aws:
  region: "us-east-1"
  profile: "default"
  
tools:
  allowed:
    - fs_read
    - fs_write
  permissions:
    fs_write:
      constraint: "workspace_only"
      
pricing:
  claude-3-sonnet:
    input_per_1k: 0.003
    output_per_1k: 0.015
```

### Basic Usage

```rust
use bedrock_agent::{Agent, Task};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize agent
    let agent = Agent::from_config("agent.yaml").await?;
    
    // Create a task
    let task = Task {
        task_id: Uuid::new_v4(),
        context: "You are a helpful assistant".to_string(),
        prompt: "Write a hello world program in Rust".to_string(),
        created_at: Utc::now(),
    };
    
    // Execute task
    let result = agent.execute_task(task).await?;
    
    // Print results
    println!("Task ID: {}", result.task_id);
    println!("Summary: {}", result.summary);
    println!("Tokens: {}", result.token_stats.total_tokens);
    println!("Cost: ${:.4}", result.cost.total_cost);
    
    Ok(())
}
```

## Environment Variables

- `HOME_DIR`: Directory for agent configuration and cache (default: `~/.bedrock-agent`)
- `WORKSPACE_DIR`: Directory for file operations (default: `./workspace`)
- `AWS_PROFILE`: AWS profile to use for authentication
- `AWS_REGION`: AWS region for Bedrock service

## Development Status

This project is under active development. See our [GitHub Issues](https://github.com/maddinenisri/bedrock-cli-agent/issues) for the current roadmap.

### Phase 1 (Weeks 1-2)
- Core Infrastructure ✅
- Basic Bedrock Integration 🚧

### Phase 2 (Weeks 3-4)
- Tool System 📋
- MCP Integration 📋

### Phase 3 (Weeks 5-6)
- Task Processing 📋
- Advanced Features 📋

### Phase 4 (Week 7)
- Observability 📋
- Testing & Documentation 📋

## Contributing

Contributions are welcome! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

This project is inspired by and leverages patterns from the [rust-bedrock-api](https://github.com/user/rust-bedrock-api) reference implementation.
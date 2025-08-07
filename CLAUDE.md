# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Bedrock CLI Agent** - A production-grade Rust AWS Bedrock LLM agent with streaming support, built-in tools, and comprehensive metrics tracking. The project uses a modular workspace architecture with specialized crates under `/crates/`.

## Quick Start

```bash
# Build and run with default config
cargo build --release
./target/release/bedrock-agent --config ./config.yaml task -p "Write hello to test.txt and read it back"

# Stream mode for real-time output
./target/release/bedrock-agent task --stream -p "List files in current directory"

# Interactive chat
./target/release/bedrock-agent chat --stream
```

## Build and Development Commands

```bash
# Build commands
cargo build --release        # Production build with optimizations
cargo build                 # Debug build for development
cargo test                  # Run all tests across workspace
cargo test -p <crate-name>  # Run tests for specific crate

# Code quality
cargo fmt                   # Format all code
cargo clippy               # Lint and check for issues
cargo clippy --fix        # Auto-fix linting issues

# Development with logging
RUST_LOG=debug cargo run -- test    # Debug logging
RUST_LOG=bedrock_agent=trace cargo run -- task -p "prompt"  # Trace specific crate

# Run examples
cargo run --example simple_task      # Simple task demonstration
cargo run --example cli_demo        # CLI functionality demo
```

## CLI Commands and Usage

### Task Execution (Single Prompt)
```bash
# Basic task
./target/release/bedrock-agent task -p "Your prompt here"

# Streaming mode (real-time output with metrics)
./target/release/bedrock-agent task --stream -p "Your prompt here"

# With additional context
./target/release/bedrock-agent task -p "Analyze this" -c "Additional context"

# With verbose logging
./target/release/bedrock-agent --verbose task -p "Debug this"
```

### Interactive Chat Mode
```bash
# Standard chat
./target/release/bedrock-agent chat

# Streaming chat (real-time responses)
./target/release/bedrock-agent chat --stream

# With custom system prompt
./target/release/bedrock-agent chat -s "You are a helpful coding assistant"
```

### Utility Commands
```bash
# List available tools
./target/release/bedrock-agent tools

# Test AWS connectivity and model access
./target/release/bedrock-agent test

# Show help
./target/release/bedrock-agent --help
```

## Test Scenarios

### File Operations
```bash
# Write and read files
cargo run -- task --stream -p "Write 'Hello World' to test.txt, then read it back"

# Multiple file operations
cargo run -- task --stream -p "Write hello to file1.txt and world to file2.txt, read both files and write combined output to file3.txt"

# Directory operations
cargo run -- task --stream -p "List all files in the current directory and count them"

# File manipulation with validation
cargo run -- task --stream -p "Create a config.json with sample data, read it back, and validate the JSON structure"
```

### Code Search and Analysis
```bash
# Search for patterns
cargo run -- task --stream -p "Find all TODO comments in the codebase"

# Grep for specific content
cargo run -- task --stream -p "Search for functions that handle streaming in the codebase"

# Find files by pattern
cargo run -- task --stream -p "Find all Rust files in the crates directory"

# Code analysis
cargo run -- task --stream -p "Analyze the main.rs file and explain its structure"
```

### System Commands
```bash
# Execute bash commands
cargo run -- task --stream -p "Show current directory and list files"

# Complex workflows
cargo run -- task --stream -p "Create a new directory called 'test_output', create three files inside it with different content, then list and read all files"

# System information
cargo run -- task --stream -p "Show system information including OS, current user, and directory"
```

### Streaming Mode Features
```bash
# Real-time output with token tracking
cargo run -- task --stream -p "Write a long story to story.txt and show progress"

# Tool execution with live feedback
cargo run -- task --stream -p "Search for all error handling in the codebase and summarize findings"

# Cost tracking demonstration
cargo run -- task --stream -p "Perform multiple operations and show the total cost"
```

## Architecture

### Workspace Crates (`/crates/`)
- **bedrock-core**: Foundation types (`Task`, `Agent` trait, `BedrockError`, `StreamResult`)
- **bedrock-client**: AWS Bedrock Converse API with streaming support
- **bedrock-config**: YAML config with `${VAR:-default}` env substitution
- **bedrock-tools**: File ops, search (grep/ripgrep), bash execution with safety
- **bedrock-task**: UUID-based task tracking and queue management
- **bedrock-agent**: Main orchestration, tool execution loop, conversation management

### Key Design Patterns
- **AWS SDK Native Types**: Direct use of `aws_sdk_bedrockruntime` types
- **Streaming with Metrics**: `StreamResult` returns both response and statistics
- **Tool Result Format**: Uses `ContentBlock::ToolResult` for proper tool responses
- **Environment Variable Substitution**: Supports `${VAR:-default}` patterns
- **Workspace Isolation**: All file operations restricted to `WORKSPACE_DIR`

## Configuration

Primary config file: `config.yaml`

```yaml
agent:
  name: bedrock-agent
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
  temperature: 0.7
  max_tokens: 4096
  
aws:
  region: "us-east-1"
  profile: "default"  # Optional
  role_arn: null     # Optional for role assumption

tools:
  allowed: 
    - fs_read
    - fs_write
    - fs_list
    - grep
    - find
    - execute_bash
  permissions:
    execute_bash:
      permission: ask  # allow, ask, or deny
      constraint: "Requires user confirmation"

pricing:
  "anthropic.claude-3-5-sonnet-20241022-v2:0":
    input_per_1k: 0.003
    output_per_1k: 0.015
    currency: USD

limits:
  max_tpm: 100000    # Tokens per minute
  max_rpm: 100       # Requests per minute  
  budget_limit: 10.0
  alert_threshold: 0.8

paths:
  home_dir: "${HOME_DIR:-~/.bedrock-agent}"
  workspace_dir: "${WORKSPACE_DIR:-./workspace}"
```

### Environment Variables
```bash
# Required
export AWS_REGION=us-east-1

# Optional AWS config
export AWS_PROFILE=your-profile
export AWS_ACCESS_KEY_ID=your-key
export AWS_SECRET_ACCESS_KEY=your-secret

# Path configuration
export WORKSPACE_DIR=/path/to/workspace
export HOME_DIR=/path/to/home

# Logging
export RUST_LOG=debug  # or trace, info, warn, error
```

## Tool System

### Built-in Tools

#### File System Tools
- **fs_read**: Read file contents (max 10MB default)
- **fs_write**: Write content to files 
- **fs_list**: List directory contents

#### Search Tools  
- **grep**: Pattern search in files (uses ripgrep)
- **find**: Find files by name patterns

#### Execution Tools
- **execute_bash**: Run bash commands with timeout

### Security Features
- Path canonicalization prevents traversal attacks
- Workspace directory enforcement
- File size limits
- Command timeout controls
- Permission system (allow/ask/deny)

## Testing Strategy

### Unit Tests
```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Test specific crate
cargo test -p bedrock-client

# Test specific function
cargo test test_streaming_response
```

### Integration Tests
```bash
# Full agent tests
cargo test -p bedrock-agent --test integration

# Tool tests
cargo test -p bedrock-tools
```

### Example Programs
```bash
# Simple task example
cargo run --example simple_task

# CLI demo
cargo run --example cli_demo
```

## AWS Integration

### Credential Chain Support
1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
2. AWS profile from `~/.aws/credentials`
3. IAM roles (EC2/ECS/Lambda)
4. IRSA for Kubernetes

### Bedrock Features Used
- **Converse API**: Multi-turn conversations
- **Streaming**: Real-time token generation
- **Tool Use**: Function calling within conversations
- **Token Usage**: Automatic token counting from API

## Cost and Metrics

### Token Tracking
```rust
pub struct TokenStatistics {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}
```

### Cost Calculation
```rust
pub struct CostDetails {
    pub model: String,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
}
```

### Streaming with Metrics
The streaming mode now returns comprehensive metrics:
```rust
pub struct StreamResult {
    pub response: String,
    pub token_stats: TokenStatistics,
    pub cost: CostDetails,
}
```

## Development Workflow

### Adding New Tools
1. Create tool in `bedrock-tools/src/tools/`
2. Implement `Tool` trait
3. Register in `ToolRegistry`
4. Add to config `allowed` list

### Modifying AWS Interaction
1. Update `bedrock-client/src/lib.rs` for API changes
2. Modify `bedrock-client/src/streaming.rs` for stream processing
3. Update message building in `bedrock-agent/src/lib.rs`

### Configuration Changes
1. Update `bedrock-config/src/lib.rs` structures
2. Modify example configs
3. Update env substitution if needed

## Important Implementation Details

### Message Format
```rust
// Tool results must be sent as User messages
let tool_result_message = Message::builder()
    .role(ConversationRole::User)
    .set_content(Some(
        tool_results.into_iter()
            .map(ContentBlock::ToolResult)
            .collect(),
    ))
    .build()
```

### Streaming Response Reconstruction
```rust
// Accumulate text and tools from stream events
match event {
    ConverseStreamOutput::ContentBlockDelta(delta) => {
        // Accumulate text chunks
    }
    ConverseStreamOutput::ContentBlockStart(start) => {
        // Start tool use accumulation
    }
    ConverseStreamOutput::ContentBlockStop(_) => {
        // Finalize tool use
    }
    ConverseStreamOutput::Metadata(metadata) => {
        // Capture token usage
    }
}
```

### Environment Variable Substitution
```rust
// Supports ${VAR:-default} pattern
static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").unwrap()
});
```

## Current Status

### Completed Features (✅)
- Core infrastructure with modular crates
- AWS Bedrock integration with streaming
- Full tool system (file, search, bash)
- Token tracking and cost calculation
- Environment variable substitution
- Streaming mode with metrics display
- Tool execution loop without repetition
- Proper AWS SDK type usage

### In Progress (🚧)
- Response caching (LRU) - Draft plan created
- Rate limiting - Issue #15
- MCP integration - Crate exists

### Known Issues and Solutions
- **Issue #36**: Fixed - Streaming now shows token stats
- **Tool Repetition**: Fixed - Proper ContentBlock::ToolResult usage
- **Variable Substitution**: Fixed - Regex-based substitution

## Performance Optimization

### Current Optimizations
- Streaming responses for low latency
- Parallel tool execution where possible
- Configuration caching
- Efficient message accumulation

### Future Optimizations
- LRU cache for non-tool responses
- Request batching for rate limits
- Connection pooling for AWS SDK

## Troubleshooting

### Common Issues

1. **AWS Credentials Not Found**
   ```bash
   aws configure
   # Or set environment variables
   export AWS_REGION=us-east-1
   export AWS_PROFILE=your-profile
   ```

2. **Model Access Denied**
   ```bash
   # Test connectivity
   ./target/release/bedrock-agent test
   # Check IAM permissions for bedrock:InvokeModel
   ```

3. **Tool Execution Failed**
   - Check workspace directory exists
   - Verify tool permissions in config
   - Check file size limits

4. **Variable Not Substituted**
   ```bash
   # Ensure variable is exported
   export WORKSPACE_DIR=/absolute/path
   # Not just set in shell
   ```

5. **Streaming Not Working**
   - Ensure using `--stream` flag
   - Check for network/proxy issues
   - Verify streaming endpoint access

## Security Considerations

- **Path Traversal**: Prevented via canonicalization
- **Command Injection**: Bash execution can require confirmation
- **File Size Limits**: Default 10MB max read
- **Workspace Isolation**: Operations restricted to WORKSPACE_DIR
- **AWS Credentials**: Follow AWS SDK best practices
- **No Secrets in Code**: Use environment variables

## Contributing Guidelines

1. **Code Style**: Run `cargo fmt` before commits
2. **Linting**: Fix all `cargo clippy` warnings
3. **Testing**: Add tests for new features
4. **Documentation**: Update this file for major changes
5. **Commits**: Use conventional commit format

## References

- [AWS Bedrock Converse API](https://docs.aws.amazon.com/bedrock/latest/userguide/model-parameters.html)
- [AWS SDK for Rust](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/welcome.html)
- [Tokio Async Runtime](https://tokio.rs/)
- [Project Issues](https://github.com/maddinenisri/bedrock-cli-agent/issues)
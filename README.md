# Bedrock CLI Agent

A Rust-based AWS Bedrock LLM agent with built-in tools, caching, and MCP integration support.

## Features

- ✅ AWS Bedrock LLM interaction via Converse API
- ✅ Full streaming support with tool execution
- ✅ AWS credential chain support (profile, IRSA, environment variables)
- ✅ Built-in file operation tools (read, write, list)
- ✅ Search tools (grep, find, ripgrep)
- ✅ Bash command execution with safety controls
- ✅ Task processing with UUID-based tracking
- ✅ Token statistics and cost tracking
- ✅ YAML-based configuration with environment variable substitution
- ✅ Modular crate architecture
- ✅ Metrics collection and monitoring
- ✅ MCP tool integration (stdio/SSE) - Tested with FIGMA and JIRA tools
- 📋 Response caching (LRU) - planned
- 📋 Rate limiting - planned

## Installation

```bash
# Build from source
cargo build --release

# Run directly
cargo run -- --help
```

## Configuration

Create a `config.yaml` file with environment variable support:

```yaml
agent:
  name: "bedrock-agent"
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
  max_tokens: 4096
  temperature: 0.7

aws:
  region: "us-east-1"
  # Optional: profile: "my-profile"

tools:
  allowed:
    - fs_read
    - fs_write
    - fs_list
    - grep
    - find
    - execute_bash

paths:
  home_dir: "${HOME_DIR:-~/.bedrock-agent}"
  workspace_dir: "${WORKSPACE_DIR:-./workspace}"

pricing:
  "anthropic.claude-3-5-sonnet-20241022-v2:0":
    input_per_1k: 0.003
    output_per_1k: 0.015
    currency: "USD"
```

## Usage

### Execute a single task
```bash
# Basic task execution
bedrock-agent task --prompt "List all Rust files in the current directory"

# With context
bedrock-agent task --prompt "Analyze this code" --context "Focus on performance"

# With streaming (full tool support)
bedrock-agent task --prompt "Write a story" --stream

# Complex multi-tool task
bedrock-agent task --prompt "Create a hello.txt file and then read it back" --stream
```

### Interactive chat mode
```bash
bedrock-agent chat

# With custom system prompt
bedrock-agent chat --system "You are a code reviewer"
```

### List available tools
```bash
bedrock-agent tools
```

### Test AWS connectivity
```bash
bedrock-agent test
```

## Environment Variables

- `HOME_DIR`: Agent configuration and cache directory (default: `~/.bedrock-agent`)
- `WORKSPACE_DIR`: Working directory for file operations (default: `./workspace`)
- `AWS_PROFILE`: AWS profile to use
- `AWS_REGION`: AWS region (overrides config)

## Architecture

The project is organized into modular crates:

- `bedrock-core`: Core types and traits
- `bedrock-client`: AWS Bedrock client implementation with streaming support
- `bedrock-config`: Configuration management with env var substitution
- `bedrock-tools`: Built-in tool implementations (fs, search, bash)
- `bedrock-task`: Task execution and queue management
- `bedrock-agent`: Main agent orchestration with tool execution loop
- `bedrock-metrics`: Token tracking, cost calculation, and metrics collection
- `bedrock-mcp`: MCP integration for external tools (stdio/SSE transports)

## Development

```bash
# Run tests
cargo test

# Run with verbose logging
RUST_LOG=debug cargo run -- test

# Format code
cargo fmt

# Check for issues
cargo clippy
```

## AWS Credentials

The agent supports the standard AWS credential chain:

1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
2. AWS profile (`~/.aws/credentials`)
3. IAM role (for EC2/ECS/Lambda)
4. IRSA (for EKS)

## Cost Tracking

The agent tracks token usage and estimates costs based on configured pricing:

- Input tokens
- Output tokens
- Total cost per request
- Model-specific pricing

## Security

- File operations are restricted to `WORKSPACE_DIR`
- Bash command execution with configurable permissions
- Tool inputs are validated
- Sensitive data is not logged
- Environment variable substitution for secure configuration

## License

MIT OR Apache-2.0
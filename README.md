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
- ✅ Conversation management (resume, export, import, delete)
- ✅ AI-powered conversation summaries
- ✅ Task continuation with context preservation
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

### CLI Command Structure

The CLI uses a unified command structure with four main command groups:

- **`conversation`** - Manage conversations (resume, summary, export, delete)
- **`task`** - Execute or manage tasks (new, resume, export)
- **`import`** - Import conversations or tasks from JSON
- **`list`** - List conversations, tasks, or show statistics

### Task Management

```bash
# Execute a new task
bedrock-agent task "List all Rust files in the current directory"

# Execute with additional context
bedrock-agent task "Analyze this code" --context "Focus on performance"

# Execute with streaming
bedrock-agent task "Write a story about AI" --stream

# Resume a task by ID
bedrock-agent task <task-id> --resume

# Resume with additional prompt
bedrock-agent task <task-id> --prompt "Continue with error handling"

# Export task to file
bedrock-agent task <task-id> --export task-backup.json
```

### Conversation Management

```bash
# Resume a conversation (default action)
bedrock-agent conversation <conversation-id>

# Resume with streaming
bedrock-agent conversation <conversation-id> --stream

# Generate AI summary of conversation
bedrock-agent conversation <conversation-id> --summary

# Export conversation to JSON
bedrock-agent conversation <conversation-id> --export backup.json

# Delete a conversation
bedrock-agent conversation <conversation-id> --delete

# Delete without confirmation
bedrock-agent conversation <conversation-id> --delete --force
```

### Listing and Statistics

```bash
# List all conversations (default)
bedrock-agent list

# List only tasks
bedrock-agent list --tasks

# List with verbose output
bedrock-agent list --verbose

# Show conversation statistics
bedrock-agent list --stats

# List all (conversations and tasks)
bedrock-agent list --list-type all
```

### Import/Export

```bash
# Import a conversation (auto-detects type)
bedrock-agent import conversation-backup.json

# Import and resume immediately
bedrock-agent import backup.json --resume

# Import with type specification
bedrock-agent import data.json --import-type conversation

# Force overwrite existing conversation
bedrock-agent import backup.json --force

# Import task and resume with streaming
bedrock-agent import task.json --import-type task --resume --stream
```

### Interactive Chat Mode

```bash
# Start interactive chat
bedrock-agent chat

# Chat with custom system prompt
bedrock-agent chat --system "You are a code reviewer"

# Chat with streaming responses
bedrock-agent chat --stream
```

### Utility Commands

```bash
# List available tools
bedrock-agent tools

# Test AWS connectivity
bedrock-agent test

# Show help for any command
bedrock-agent <command> --help
```

## Practical Examples

### Example 1: Code Analysis Workflow
```bash
# Analyze a codebase
bedrock-agent task "Analyze the Rust codebase and identify performance bottlenecks"

# Export the analysis for later reference
bedrock-agent task <task-id> --export analysis.json

# Continue with specific improvements
bedrock-agent task <task-id> --prompt "Focus on the database query optimization"
```

### Example 2: Conversation-Based Development
```bash
# Start a conversation about a feature
bedrock-agent chat
> Help me design a REST API for user management
> What authentication method should I use?
> Can you generate the OpenAPI spec?

# Export the conversation for documentation
bedrock-agent conversation <id> --export api-design.json

# Generate a summary for the team
bedrock-agent conversation <id> --summary > api-design-summary.md
```

### Example 3: Backup and Restore Workflow
```bash
# Export all conversations for backup
for id in $(bedrock-agent list | grep -E '^[a-f0-9-]{36}' | awk '{print $1}'); do
  bedrock-agent conversation $id --export "backup/$id.json"
done

# Import conversations on another machine
for file in backup/*.json; do
  bedrock-agent import "$file"
done
```

### Example 4: Task Tracking and Reporting
```bash
# View all tasks with status
bedrock-agent list --tasks --verbose

# Get statistics for the week
bedrock-agent list --stats

# Export specific task results
bedrock-agent task <task-id> --export "reports/task-$(date +%Y%m%d).json"
```

## Migration Guide

If you're upgrading from an older version with separate commands, here's how to migrate:

### Old Commands → New Commands

| Old Command | New Command |
|------------|------------|
| `bedrock-agent resume <id>` | `bedrock-agent conversation <id>` |
| `bedrock-agent list-conversations` | `bedrock-agent list` |
| `bedrock-agent export-conversation <id>` | `bedrock-agent conversation <id> --export <file>` |
| `bedrock-agent delete-conversation <id>` | `bedrock-agent conversation <id> --delete` |
| `bedrock-agent conversation-stats` | `bedrock-agent list --stats` |
| `bedrock-agent resume-task <id>` | `bedrock-agent task <id> --resume` |
| `bedrock-agent import-conversation <file>` | `bedrock-agent import <file>` |
| `bedrock-agent import-task <file>` | `bedrock-agent import <file> --import-type task` |

### Key Changes

1. **Unified Command Structure**: Commands are now grouped logically under `conversation`, `task`, `import`, and `list`
2. **Auto-Detection**: The CLI can automatically detect UUIDs vs prompts, and conversation vs task imports
3. **Chained Options**: Multiple operations can be combined (e.g., `--summary --export`)
4. **Better Defaults**: Resume is the default action for conversations

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
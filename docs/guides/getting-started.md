# Getting Started with Bedrock CLI Agent

This guide will help you get up and running with the Bedrock CLI Agent, from installation through your first AI-powered task.

## Prerequisites

Before you begin, ensure you have the following:

### Required
- **Rust** (1.70 or later) - [Install Rust](https://rustup.rs/)
- **AWS Account** with Bedrock access enabled
- **AWS Credentials** configured (see [AWS Setup](#aws-setup))
- **Git** for cloning the repository

### Optional
- **Ripgrep** for enhanced search capabilities (`brew install ripgrep` on macOS)
- **Docker** if using containerized deployment

## Installation

### Step 1: Clone the Repository

```bash
git clone https://github.com/maddinenisri/bedrock-cli-agent.git
cd bedrock-cli-agent
```

### Step 2: Build from Source

```bash
# Build in release mode for best performance
cargo build --release

# The binary will be available at:
# target/release/bedrock-agent
```

### Step 3: Add to PATH (Optional)

```bash
# Add to your shell profile (.bashrc, .zshrc, etc.)
export PATH="$PATH:/path/to/bedrock-cli-agent/target/release"

# Or create a symbolic link
sudo ln -s /path/to/bedrock-cli-agent/target/release/bedrock-agent /usr/local/bin/bedrock-agent
```

## AWS Setup

### Option 1: AWS Profile (Recommended for Development)

1. Configure AWS CLI profile:
```bash
aws configure --profile bedrock-dev
# Enter your AWS Access Key ID
# Enter your AWS Secret Access Key
# Enter default region (e.g., us-east-1)
# Enter default output format (json)
```

2. Set the profile in your environment:
```bash
export AWS_PROFILE=bedrock-dev
```

### Option 2: Environment Variables

```bash
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export AWS_REGION="us-east-1"
```

### Option 3: IAM Role (For EC2/ECS/Lambda)

If running on AWS infrastructure, the agent will automatically use the instance/task role.

### Required AWS Permissions

Ensure your AWS credentials have the following permissions:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "bedrock:InvokeModel",
        "bedrock:InvokeModelWithResponseStream"
      ],
      "Resource": "arn:aws:bedrock:*:*:foundation-model/*"
    }
  ]
}
```

## Configuration

### Step 1: Create Configuration File

Create a `config.yaml` file in your home directory or project root:

```bash
mkdir -p ~/.bedrock-agent
nano ~/.bedrock-agent/config.yaml
```

### Step 2: Basic Configuration

```yaml
# ~/.bedrock-agent/config.yaml
agent:
  name: "my-bedrock-agent"
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
  max_tokens: 4096
  temperature: 0.7

aws:
  region: "us-east-1"
  # profile: "bedrock-dev"  # Optional: specify AWS profile

tools:
  allowed:
    - fs_read
    - fs_write
    - fs_list
    - grep
    - find
    - execute_bash

paths:
  home_dir: "${HOME}/.bedrock-agent"
  workspace_dir: "${HOME}/bedrock-workspace"

pricing:
  "anthropic.claude-3-5-sonnet-20241022-v2:0":
    input_per_1k: 0.003
    output_per_1k: 0.015
    currency: "USD"
```

### Step 3: Create Workspace Directory

```bash
# Create a safe workspace for file operations
mkdir -p ~/bedrock-workspace
```

## Quick Test

### Verify Installation

```bash
# Check if the agent is properly installed
bedrock-agent --version

# Test AWS connectivity
bedrock-agent test
```

Expected output:
```
✓ AWS credentials configured
✓ Bedrock client initialized
✓ Model available: anthropic.claude-3-5-sonnet-20241022-v2:0
✓ Tool registry initialized with 6 tools
✓ Configuration valid
```

## Your First Task

### Example 1: Simple Question

```bash
bedrock-agent task --prompt "What is the capital of France?"
```

### Example 2: File Operations

```bash
# Create a file
bedrock-agent task --prompt "Create a file called hello.txt with 'Hello, World!' content"

# Read the file
bedrock-agent task --prompt "Read the contents of hello.txt"
```

### Example 3: Code Analysis

```bash
# Analyze Python files in current directory
bedrock-agent task --prompt "List all Python files and summarize what each one does"
```

### Example 4: Interactive Chat

```bash
# Start interactive chat mode
bedrock-agent chat

# With custom system prompt
bedrock-agent chat --system "You are a helpful Python programming assistant"
```

## Common Use Cases

### 1. Code Review

```bash
bedrock-agent task --prompt "Review this code for security issues" \
  --context "Focus on SQL injection and XSS vulnerabilities"
```

### 2. Documentation Generation

```bash
bedrock-agent task --prompt "Generate API documentation for all functions in src/"
```

### 3. File Organization

```bash
bedrock-agent task --prompt "Organize files in the current directory by type and create a summary report"
```

### 4. Search and Analysis

```bash
bedrock-agent task --prompt "Find all TODO comments in the codebase and create a task list"
```

### 5. Automated Testing

```bash
bedrock-agent task --prompt "Write unit tests for the functions in utils.py"
```

## Streaming Mode

For long responses, use streaming mode to see output in real-time:

```bash
bedrock-agent task --prompt "Write a detailed analysis of the codebase" --stream
```

## Working with Tools

### Available Built-in Tools

| Tool | Description | Example Usage |
|------|-------------|---------------|
| `fs_read` | Read file contents | "Read config.yaml" |
| `fs_write` | Write to files | "Create a README.md" |
| `fs_list` | List directory contents | "List all files in src/" |
| `grep` | Search with patterns | "Find all error messages" |
| `find` | Find files by name | "Find all .py files" |
| `execute_bash` | Run shell commands | "Run npm test" |

### Tool Permissions

Control which tools the agent can use in `config.yaml`:

```yaml
tools:
  allowed:
    - fs_read      # Safe: read-only
    - fs_list      # Safe: read-only
    - grep         # Safe: read-only
    - find         # Safe: read-only
    # - fs_write   # Caution: modifies files
    # - execute_bash # Caution: runs commands
```

## Environment Variables

### Configuration Variables

```bash
# Set custom config location
export BEDROCK_CONFIG_PATH="~/my-config.yaml"

# Set workspace directory
export WORKSPACE_DIR="/path/to/safe/workspace"

# Set home directory for agent data
export HOME_DIR="~/.bedrock-agent"
```

### Debugging

```bash
# Enable debug logging
export RUST_LOG=debug

# Run with verbose output
bedrock-agent task --prompt "test" --verbose
```

## Troubleshooting

### Issue: "AWS credentials not found"

**Solution**: Ensure AWS credentials are configured:
```bash
aws sts get-caller-identity
```

### Issue: "Model not available in region"

**Solution**: Check model availability:
```bash
aws bedrock list-foundation-models --region us-east-1 | grep claude
```

### Issue: "Permission denied for file operations"

**Solution**: Ensure workspace directory exists and has proper permissions:
```bash
mkdir -p ~/bedrock-workspace
chmod 755 ~/bedrock-workspace
```

### Issue: "Tool execution failed"

**Solution**: Check tool permissions in config.yaml and ensure required tools are in the allowed list.

### Issue: "Rate limit exceeded"

**Solution**: The agent doesn't have built-in rate limiting yet. Space out requests or implement client-side delays.

## Best Practices

### 1. Security

- Always use workspace sandboxing for file operations
- Be cautious with `execute_bash` tool
- Review generated code before execution
- Don't store sensitive data in prompts

### 2. Cost Optimization

- Use appropriate `max_tokens` limits
- Be specific in prompts to reduce back-and-forth
- Monitor token usage with built-in metrics
- Consider caching for repeated queries (coming soon)

### 3. Prompt Engineering

```bash
# Be specific
❌ "Fix the code"
✅ "Fix the syntax error in line 42 of main.py"

# Provide context
❌ "Write tests"
✅ "Write unit tests for the User class in models.py using pytest"

# Set boundaries
❌ "Analyze all files"
✅ "Analyze Python files in the src/ directory, focusing on error handling"
```

### 4. Tool Usage

- Start with read-only tools for exploration
- Use `fs_list` before `fs_read` to understand structure
- Validate paths exist before writing
- Use streaming for long operations

## Advanced Usage

### Custom System Prompts

```bash
# For code review
bedrock-agent chat --system "You are a senior software engineer reviewing code for production readiness"

# For documentation
bedrock-agent chat --system "You are a technical writer creating clear, concise documentation"

# For debugging
bedrock-agent chat --system "You are a debugging assistant helping identify and fix issues"
```

### Batch Processing

```bash
# Process multiple files
for file in *.py; do
  bedrock-agent task --prompt "Add type hints to $file"
done
```

### Integration with Scripts

```python
#!/usr/bin/env python3
import subprocess
import json

def run_bedrock_task(prompt):
    result = subprocess.run(
        ["bedrock-agent", "task", "--prompt", prompt],
        capture_output=True,
        text=True
    )
    return result.stdout

# Example usage
analysis = run_bedrock_task("Analyze code complexity in main.py")
print(analysis)
```

## Next Steps

Now that you have the basics working:

1. **Explore Tools**: Try different built-in tools with various prompts
2. **Customize Configuration**: Adjust temperature, max_tokens, and model settings
3. **Read Documentation**: 
   - [Configuration Guide](configuration.md) for advanced settings
   - [API Documentation](../api/crates/) for programmatic usage
   - [Examples](examples.md) for more use cases
4. **Monitor Usage**: Track token costs and optimize prompts
5. **Report Issues**: Found a bug? Check [Known Issues](../status/KNOWN_ISSUES.md)

## Getting Help

- **Documentation**: See the [main documentation index](../README.md)
- **Issues**: Report bugs on [GitHub Issues](https://github.com/maddinenisri/bedrock-cli-agent/issues)
- **Examples**: Check the `/examples` directory for sample code
- **Troubleshooting**: See the [Troubleshooting Guide](troubleshooting.md)

---

**Pro Tip**: Start with read-only operations (`fs_read`, `grep`, `find`) to explore safely before using write operations or command execution.

---

*Happy coding with your AI assistant! 🤖*
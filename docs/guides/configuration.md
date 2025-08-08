# Configuration Guide

This guide provides detailed information about configuring the Bedrock CLI Agent for various use cases and environments.

## Configuration Overview

The Bedrock CLI Agent uses a YAML-based configuration system with support for:
- Environment variable substitution
- Default values
- Hierarchical configuration loading
- Runtime overrides

## Configuration File Locations

The agent searches for configuration in the following order (first found wins):

1. Path specified by `--config` flag
2. `BEDROCK_CONFIG_PATH` environment variable
3. `./config.yaml` (current directory)
4. `~/.bedrock-agent/config.yaml` (user home)
5. `/etc/bedrock-agent/config.yaml` (system-wide)

## Complete Configuration Reference

```yaml
# Complete configuration with all options
agent:
  # Agent identifier (used in logs and metrics)
  name: "my-bedrock-agent"
  
  # Model selection - see Available Models section
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
  
  # Maximum tokens for response (1-4096)
  max_tokens: 4096
  
  # Temperature for response creativity (0.0-1.0)
  # 0.0 = deterministic, 1.0 = creative
  temperature: 0.7
  
  # Top-p sampling (0.0-1.0)
  top_p: 0.9
  
  # System prompt (optional)
  system_prompt: "You are a helpful AI assistant"
  
  # Request timeout in seconds
  timeout: 300
  
  # Retry configuration
  retry:
    max_attempts: 3
    initial_delay: 1000  # milliseconds
    max_delay: 10000     # milliseconds
    
aws:
  # AWS region for Bedrock
  region: "us-east-1"
  
  # AWS profile name (optional)
  # profile: "bedrock-dev"
  
  # Endpoint URL override (optional, for testing)
  # endpoint_url: "https://bedrock.us-east-1.amazonaws.com"
  
  # Credential configuration (optional, uses default chain if not specified)
  credentials:
    # Option 1: Static credentials (not recommended for production)
    # access_key_id: "${AWS_ACCESS_KEY_ID}"
    # secret_access_key: "${AWS_SECRET_ACCESS_KEY}"
    # session_token: "${AWS_SESSION_TOKEN}"  # Optional, for temporary credentials
    
    # Option 2: Profile
    # profile: "bedrock-dev"
    
    # Option 3: Role ARN (for cross-account access)
    # role_arn: "arn:aws:iam::123456789012:role/BedrockRole"
    # role_session_name: "bedrock-agent-session"

tools:
  # List of allowed tools
  allowed:
    - fs_read
    - fs_write
    - fs_list
    - grep
    - find
    - ripgrep
    - execute_bash
  
  # Tool-specific permissions (optional)
  permissions:
    fs_read:
      permission: "allow"
      constraints:
        max_file_size: 10485760  # 10MB in bytes
        allowed_extensions: [".txt", ".md", ".json", ".yaml", ".toml"]
    
    fs_write:
      permission: "allow"
      constraints:
        max_file_size: 5242880   # 5MB in bytes
        forbidden_paths: ["/etc", "/sys", "/proc"]
    
    execute_bash:
      permission: "allow"
      constraints:
        timeout: 30000  # milliseconds
        allowed_commands: ["ls", "grep", "find", "echo", "cat"]
        forbidden_commands: ["rm", "sudo", "chmod", "chown"]
        
  # Global tool settings
  settings:
    default_timeout: 30000      # milliseconds
    max_concurrent_tools: 5
    enable_tool_logging: true

paths:
  # Home directory for agent data (cache, logs, etc.)
  home_dir: "${HOME}/.bedrock-agent"
  
  # Workspace directory for file operations
  workspace_dir: "${WORKSPACE_DIR:-./workspace}"
  
  # Cache directory (optional, defaults to home_dir/cache)
  cache_dir: "${HOME}/.bedrock-agent/cache"
  
  # Log directory (optional, defaults to home_dir/logs)
  log_dir: "${HOME}/.bedrock-agent/logs"

# Model pricing configuration
pricing:
  "anthropic.claude-3-5-sonnet-20241022-v2:0":
    input_per_1k: 0.003
    output_per_1k: 0.015
    currency: "USD"
  
  "anthropic.claude-3-opus-20240229-v1:0":
    input_per_1k: 0.015
    output_per_1k: 0.075
    currency: "USD"
  
  "anthropic.claude-3-haiku-20240307-v1:0":
    input_per_1k: 0.00025
    output_per_1k: 0.00125
    currency: "USD"
  
  "meta.llama3-70b-instruct-v1:0":
    input_per_1k: 0.00265
    output_per_1k: 0.0035
    currency: "USD"

# Caching configuration (planned feature)
cache:
  enabled: false  # Not yet implemented
  type: "lru"
  max_size: 1073741824  # 1GB in bytes
  ttl: 3600  # seconds
  compression: true

# Rate limiting configuration (planned feature)
rate_limiting:
  enabled: false  # Not yet implemented
  limits:
    - model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
      tpm: 100000  # tokens per minute
      rpm: 100     # requests per minute
    - model: "default"
      tpm: 50000
      rpm: 50

# MCP (Model Context Protocol) configuration
# WARNING: MCP has critical issues - see docs/implementation/mcp/known-issues.md
mcp:
  enabled: false  # Keep disabled due to critical bugs
  # config_files:
  #   - ~/.bedrock-agent/mcp/*.yaml
  # servers:
  #   - filesystem
  #   - github

# Logging configuration
logging:
  level: "info"  # trace, debug, info, warn, error
  format: "json"  # json, pretty, compact
  file:
    enabled: true
    path: "${HOME}/.bedrock-agent/logs/agent.log"
    rotation: "daily"  # daily, size, never
    max_size: 104857600  # 100MB (for size rotation)
    max_age: 7  # days
    max_backups: 5
  
  console:
    enabled: true
    format: "pretty"

# Metrics configuration
metrics:
  enabled: true
  export:
    type: "file"  # file, prometheus, cloudwatch
    path: "${HOME}/.bedrock-agent/metrics/metrics.json"
    interval: 60  # seconds
  
  track:
    tokens: true
    costs: true
    latency: true
    tools: true
    errors: true

# Security configuration
security:
  # Sandbox file operations
  sandbox_enabled: true
  
  # Validate all paths
  path_validation: "strict"  # strict, normal, permissive
  
  # Command execution safety
  command_execution:
    enabled: true
    require_approval: false  # Set to true for interactive approval
    shell_detection: true    # Detect shell operators (|, >, <, etc.)
    
  # Sensitive data handling
  redact_secrets: true
  secret_patterns:
    - "(?i)(api[_-]?key|secret|token|password)\\s*[:=]\\s*['\"]?([^'\"\\s]+)"
```

## Environment Variable Substitution

The configuration supports environment variable substitution with two formats:

### Basic Substitution
```yaml
region: "${AWS_REGION}"
```

### With Default Values
```yaml
workspace_dir: "${WORKSPACE_DIR:-/tmp/workspace}"
```

### Available Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `HOME` | User home directory | System dependent |
| `USER` | Current username | System dependent |
| `WORKSPACE_DIR` | Tool workspace directory | `./workspace` |
| `HOME_DIR` | Agent home directory | `~/.bedrock-agent` |
| `AWS_REGION` | AWS region | `us-east-1` |
| `AWS_PROFILE` | AWS profile name | None |
| `BEDROCK_CONFIG_PATH` | Config file path | See search order |
| `RUST_LOG` | Logging level | `info` |

## Configuration Profiles

### Minimal Configuration

```yaml
# Minimal required configuration
agent:
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"

aws:
  region: "us-east-1"

paths:
  workspace_dir: "./workspace"
```

### Development Configuration

```yaml
# Development configuration with debugging
agent:
  name: "dev-agent"
  model: "anthropic.claude-3-haiku-20240307-v1:0"  # Cheaper model
  max_tokens: 2048
  temperature: 0.5

aws:
  region: "us-east-1"
  profile: "development"

tools:
  allowed:
    - fs_read
    - fs_list
    - grep
    - find
    # Exclude write operations in dev

logging:
  level: "debug"
  format: "pretty"

metrics:
  enabled: true
  track:
    tokens: true
    costs: true
```

### Production Configuration

```yaml
# Production configuration with security
agent:
  name: "prod-agent"
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
  max_tokens: 4096
  temperature: 0.3  # More deterministic
  timeout: 600
  retry:
    max_attempts: 5
    initial_delay: 2000

aws:
  region: "${AWS_REGION}"
  credentials:
    role_arn: "${BEDROCK_ROLE_ARN}"

tools:
  allowed:
    - fs_read
    - fs_write
    - fs_list
    - grep
    - find
  permissions:
    fs_write:
      permission: "allow"
      constraints:
        forbidden_paths: ["/", "/etc", "/sys", "/proc", "/boot"]
    execute_bash:
      permission: "deny"  # No command execution in production

paths:
  home_dir: "/var/lib/bedrock-agent"
  workspace_dir: "/var/lib/bedrock-agent/workspace"

logging:
  level: "warn"
  format: "json"
  file:
    enabled: true
    rotation: "size"
    max_size: 104857600  # 100MB

metrics:
  enabled: true
  export:
    type: "cloudwatch"

security:
  sandbox_enabled: true
  path_validation: "strict"
  command_execution:
    enabled: false
  redact_secrets: true
```

### Cost-Optimized Configuration

```yaml
# Configuration optimized for cost
agent:
  name: "cost-optimized"
  model: "anthropic.claude-3-haiku-20240307-v1:0"  # Cheapest model
  max_tokens: 1024  # Limit response size
  temperature: 0.3

aws:
  region: "us-east-1"

tools:
  allowed:
    - fs_read
    - grep
    # Minimal tool set

# When implemented, these will help:
cache:
  enabled: true
  type: "lru"
  ttl: 7200  # 2 hours

rate_limiting:
  enabled: true
  limits:
    - model: "default"
      tpm: 10000  # Conservative limit
      rpm: 20
```

## Available Models

### Claude Models (Anthropic)

| Model ID | Description | Cost (per 1K tokens) | Use Case |
|----------|-------------|---------------------|----------|
| `anthropic.claude-3-5-sonnet-20241022-v2:0` | Most capable, balanced | $0.003/$0.015 | General purpose |
| `anthropic.claude-3-opus-20240229-v1:0` | Most powerful | $0.015/$0.075 | Complex tasks |
| `anthropic.claude-3-haiku-20240307-v1:0` | Fast and cheap | $0.00025/$0.00125 | Simple tasks |

### Llama Models (Meta)

| Model ID | Description | Cost (per 1K tokens) | Use Case |
|----------|-------------|---------------------|----------|
| `meta.llama3-70b-instruct-v1:0` | Open source, capable | $0.00265/$0.0035 | General purpose |
| `meta.llama3-8b-instruct-v1:0` | Smaller, faster | $0.0003/$0.0006 | Simple tasks |

## Tool Configuration

### Tool Permissions

Each tool can have specific permissions:

```yaml
tools:
  permissions:
    tool_name:
      permission: "allow"  # allow, deny, ask
      constraints:
        # Tool-specific constraints
```

### File System Tools

```yaml
tools:
  permissions:
    fs_read:
      permission: "allow"
      constraints:
        max_file_size: 10485760  # 10MB
        allowed_extensions: [".txt", ".md", ".json"]
        forbidden_paths: ["/etc/passwd", "/etc/shadow"]
    
    fs_write:
      permission: "allow"
      constraints:
        max_file_size: 5242880  # 5MB
        allowed_paths: ["${WORKSPACE_DIR}"]
        create_directories: true
```

### Search Tools

```yaml
tools:
  permissions:
    grep:
      permission: "allow"
      constraints:
        max_results: 1000
        timeout: 10000
        exclude_patterns: ["*.log", "*.tmp"]
    
    ripgrep:
      permission: "allow"
      constraints:
        max_filesize: 5242880  # Skip files larger than 5MB
        follow_links: false
        hidden: false  # Don't search hidden files
```

### Command Execution

```yaml
tools:
  permissions:
    execute_bash:
      permission: "allow"
      constraints:
        timeout: 30000
        allowed_commands: ["ls", "echo", "pwd", "date"]
        forbidden_commands: ["sudo", "rm", "format", "kill"]
        allowed_directories: ["${WORKSPACE_DIR}"]
        env_vars:
          PATH: "/usr/local/bin:/usr/bin:/bin"
          USER: "bedrock-agent"
```

## Security Best Practices

### 1. Workspace Isolation

Always use workspace sandboxing:

```yaml
paths:
  workspace_dir: "/isolated/workspace"

security:
  sandbox_enabled: true
  path_validation: "strict"
```

### 2. Tool Restrictions

Limit tools based on environment:

```yaml
# Development
tools:
  allowed: ["fs_read", "grep", "find"]

# Production
tools:
  allowed: ["fs_read"]  # Read-only
```

### 3. Credential Management

Never hardcode credentials:

```yaml
# Bad
aws:
  credentials:
    access_key_id: "AKIAIOSFODNN7EXAMPLE"  # Never do this!

# Good
aws:
  credentials:
    profile: "${AWS_PROFILE}"
    # Or use IAM roles
```

### 4. Logging Security

```yaml
security:
  redact_secrets: true
  secret_patterns:
    - "(?i)api[_-]?key"
    - "(?i)secret"
    - "(?i)token"
    - "(?i)password"
```

## Performance Tuning

### 1. Token Optimization

```yaml
agent:
  max_tokens: 2048  # Reduce for faster responses
  temperature: 0.3   # Lower for more focused responses
```

### 2. Timeout Configuration

```yaml
agent:
  timeout: 120  # Shorter timeout for quick tasks

tools:
  settings:
    default_timeout: 10000  # 10 seconds for tools
```

### 3. Concurrent Operations

```yaml
tools:
  settings:
    max_concurrent_tools: 3  # Limit parallelism
```

## Monitoring and Metrics

### Enable Comprehensive Metrics

```yaml
metrics:
  enabled: true
  track:
    tokens: true
    costs: true
    latency: true
    tools: true
    errors: true
  export:
    type: "file"
    path: "./metrics/agent-metrics.json"
    interval: 60
```

### Metrics Output Format

```json
{
  "timestamp": "2024-01-01T12:00:00Z",
  "tokens": {
    "input": 1500,
    "output": 500,
    "total": 2000
  },
  "cost": {
    "amount": 0.0105,
    "currency": "USD"
  },
  "latency": {
    "total_ms": 3500,
    "model_ms": 3000,
    "tools_ms": 500
  }
}
```

## Troubleshooting Configuration

### Debug Configuration Loading

```bash
# Show resolved configuration
RUST_LOG=debug bedrock-agent test

# Test specific config file
bedrock-agent --config ./test-config.yaml test
```

### Common Issues

#### Issue: Configuration not found

```bash
# Check search paths
bedrock-agent --help | grep config

# Explicitly specify config
export BEDROCK_CONFIG_PATH=/path/to/config.yaml
```

#### Issue: Environment variables not substituted

```yaml
# Ensure variables are exported
export WORKSPACE_DIR=/my/workspace

# Use default values
workspace_dir: "${WORKSPACE_DIR:-/tmp/workspace}"
```

#### Issue: Tool permissions denied

```yaml
# Check tool is in allowed list
tools:
  allowed:
    - tool_name

# Check specific permissions
tools:
  permissions:
    tool_name:
      permission: "allow"  # Not "deny"
```

## Migration Guide

### From Version 0.x to 1.x

```yaml
# Old format (0.x)
bedrock:
  model: "claude-v3"
  
# New format (1.x)
agent:
  model: "anthropic.claude-3-5-sonnet-20241022-v2:0"
```

## Advanced Configuration

### Multi-Environment Setup

Create separate configs for each environment:

```bash
# Structure
~/.bedrock-agent/
├── config.yaml          # Default
├── config.dev.yaml      # Development
├── config.staging.yaml  # Staging
└── config.prod.yaml     # Production

# Usage
bedrock-agent --config ~/.bedrock-agent/config.dev.yaml
```

### Dynamic Configuration

Use environment variables for dynamic values:

```yaml
agent:
  model: "${BEDROCK_MODEL:-anthropic.claude-3-5-sonnet-20241022-v2:0}"
  max_tokens: "${MAX_TOKENS:-4096}"
  temperature: "${TEMPERATURE:-0.7}"
```

### Configuration Validation

The agent validates configuration on startup:

1. Required fields present
2. Value ranges valid
3. Paths accessible
4. AWS credentials valid
5. Model available in region

## Configuration Schema

For tooling integration, the configuration schema is available at:
- JSON Schema: `schemas/config.schema.json`
- TypeScript: `schemas/config.d.ts`

---

*For more information, see the [Getting Started Guide](getting-started.md) or [Troubleshooting Guide](troubleshooting.md).*
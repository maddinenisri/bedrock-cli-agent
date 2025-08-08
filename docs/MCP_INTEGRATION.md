# MCP (Model Context Protocol) Integration

## Overview

The Bedrock CLI Agent now supports the Model Context Protocol (MCP), enabling integration with external tool servers. MCP allows the agent to discover and use tools from various sources beyond the built-in tool set.

## Features

- **Stdio Transport**: Connect to process-based MCP servers via stdin/stdout
- **SSE Transport**: Connect to HTTP-based MCP servers using Server-Sent Events
- **Automatic Tool Discovery**: Tools from MCP servers are automatically registered
- **Health Monitoring**: Configurable health checks with automatic restart
- **Flexible Configuration**: Support for inline configs, separate files, and environment variables
- **Secret Management**: Support for environment variables, files, and future vault integration

## Configuration

### Basic Configuration

Add MCP settings to your `config.yaml`:

```yaml
mcp:
  enabled: true
  config_files:
    - ~/.bedrock-agent/mcp/*.yaml
  servers:
    - filesystem
    - github
```

### Inline MCP Server Configuration

Define MCP servers directly in your agent config:

```yaml
mcp:
  enabled: true
  servers:
    - my-server
  inline_servers:
    my-server:
      command: npx
      args: ["@modelcontextprotocol/server-filesystem", "--stdio"]
      env:
        WORKSPACE: "${WORKSPACE_DIR}"
      timeout: 30000
```

### Separate MCP Configuration File

Create `~/.bedrock-agent/mcp/servers.yaml`:

```yaml
mcpServers:
  filesystem:
    command: npx
    args: ["@modelcontextprotocol/server-filesystem", "--stdio"]
    env:
      WORKSPACE: "${WORKSPACE_DIR}"
    health_check:
      interval: 60
      max_failures: 3
```

## Transport Types

### Stdio Transport

For process-based MCP servers:

```yaml
filesystem:
  command: /usr/local/bin/mcp-server
  args: ["--stdio"]
  env:
    CONFIG: "/etc/mcp/config.json"
  timeout: 30000
```

### SSE Transport

For HTTP-based MCP servers:

```yaml
api-server:
  type: sse
  url: http://localhost:8080
  headers:
    Authorization: "Bearer ${API_TOKEN}"
  timeout: 60000
```

## Health Monitoring

Configure health checks to monitor MCP server status:

```yaml
health_check:
  interval: 60        # Check every 60 seconds
  timeout: 5          # Health check timeout
  max_failures: 3     # Restart after 3 failures
```

## Restart Policy

Configure automatic restart behavior:

```yaml
restart_policy:
  max_retries: 3
  initial_delay: 1    # Seconds
  max_delay: 30       # Maximum backoff delay
  backoff: exponential  # linear, exponential, or fixed
```

## Environment Variables and Secrets

### Environment Variables

```yaml
env:
  API_KEY: "${API_KEY}"                    # Simple substitution
  WORKSPACE: "${WORKSPACE_DIR:-./default}" # With default value
```

### File-based Secrets

```yaml
headers:
  token: "${file:~/.secrets/token.txt}"    # Read from file
```

### Future: Vault Integration

```yaml
env:
  SECRET: "${vault:secret/path}"           # Coming soon
```

## Tool Naming

MCP tools are registered with their simple names (without server prefix) for Bedrock compatibility. If there are naming conflicts, the first registered tool takes precedence.

## Examples

### Example 1: Filesystem Tools

```yaml
# config.yaml
mcp:
  enabled: true
  servers:
    - filesystem
  inline_servers:
    filesystem:
      command: npx
      args: ["@modelcontextprotocol/server-filesystem", "--stdio"]
      env:
        WORKSPACE: "./workspace"
```

Usage:
```
> List all files in the workspace
> Create a file called notes.txt
> Read the contents of notes.txt
```

### Example 2: GitHub Integration

```yaml
# config.yaml
mcp:
  enabled: true
  servers:
    - github
  inline_servers:
    github:
      command: npx
      args: ["@modelcontextprotocol/server-github", "--stdio"]
      env:
        GITHUB_TOKEN: "${GITHUB_TOKEN}"
```

Usage:
```
> List my recent GitHub repositories
> Show open issues in bedrock-cli-agent
> Create an issue titled "Test MCP Integration"
```

### Example 3: Multiple Servers

```yaml
# config.yaml
mcp:
  enabled: true
  config_files:
    - ./mcp-servers.yaml
  servers:
    - filesystem
    - database
    - custom-api
```

## Running the Examples

1. **Basic MCP Demo**:
   ```bash
   cargo run --example mcp_demo
   ```

2. **Stdio Transport Test**:
   ```bash
   cargo run -- --config examples/mcp-stdio-test.yaml task -p "List files"
   ```

3. **SSE Transport Test**:
   ```bash
   cargo run -- --config examples/mcp-sse-test.yaml chat --stream
   ```

## Troubleshooting

### MCP Server Not Starting

1. Check the command exists:
   ```bash
   which npx  # or your command
   ```

2. Verify environment variables:
   ```bash
   echo $GITHUB_TOKEN
   ```

3. Check logs:
   ```bash
   RUST_LOG=bedrock_mcp=debug cargo run
   ```

### Tools Not Available

1. Verify MCP is enabled:
   ```yaml
   mcp:
     enabled: true
   ```

2. Check server is in the servers list:
   ```yaml
   servers:
     - your-server
   ```

3. Ensure server is not disabled:
   ```yaml
   your-server:
     disabled: false  # or remove this line
   ```

### Health Check Failures

If a server keeps failing health checks:

1. Increase the timeout:
   ```yaml
   health_check:
     timeout: 10  # Increase from 5
   ```

2. Check server logs for errors

3. Verify network connectivity (for SSE)

## API Reference

### Agent Methods

```rust
// List connected MCP servers
let servers = agent.list_mcp_servers().await;

// Shutdown MCP servers cleanly
agent.shutdown().await?;
```

### McpManager

```rust
// Create manager
let manager = McpManager::new(tool_registry);

// Load configurations
manager.load_config_file("mcp.yaml").await?;

// Start servers
manager.start_servers(vec!["server1", "server2"]).await?;

// Stop all servers
manager.stop_all().await?;
```

## Security Considerations

1. **Command Execution**: MCP servers run as child processes with the same privileges as the agent
2. **Environment Variables**: Sensitive values should use `${env:VAR}` or `${file:path}` patterns
3. **Network Security**: SSE connections should use HTTPS in production
4. **Tool Permissions**: MCP tools respect the same permission system as built-in tools

## Limitations

- Tool names cannot contain server prefixes due to Bedrock compatibility
- Maximum 10 iterations for tool execution loops
- SSE transport requires the server to send an "endpoint" event or defaults to `/messages`
- Health monitoring runs in background tasks and may have slight delays

## Future Enhancements

- [ ] WebSocket transport support
- [ ] Tool caching for frequently used operations
- [ ] Vault integration for secret management
- [ ] Tool usage analytics
- [ ] Dynamic tool updates without restart
- [ ] Tool result caching based on semantic analysis
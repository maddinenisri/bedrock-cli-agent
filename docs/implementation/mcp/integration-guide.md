# MCP Integration Guide

This guide explains how to integrate and use MCP (Model Context Protocol) servers with the Bedrock CLI Agent.

## Overview

MCP integration is fully functional and has been successfully tested with multiple external tool servers including FIGMA (stdio) and JIRA (SSE).

## Tested Integrations

### ✅ FIGMA Tools (Stdio Transport)
- **Transport**: Process-based communication via stdin/stdout
- **Status**: Fully working
- **Features**: Design file operations, component management

### ✅ JIRA Tools (SSE Transport via Redux HTTP API)
- **Transport**: Server-Sent Events over HTTP
- **Status**: Fully working
- **Features**: Issue management, project operations

## Configuration Examples

### 1. FIGMA Integration (Stdio)

```yaml
mcp:
  enabled: true
  servers:
    - figma-tools
  inline_servers:
    figma-tools:
      command: npx
      args: ["-y", "figma-developer-mcp", "--stdio"]
      env:
        FIGMA_API_KEY: "${FIGMA_API_KEY}"
      timeout: 30000
      health_check:
        interval: 60
        max_failures: 3
```

### 2. JIRA Integration (SSE via Redux)

```yaml
mcp:
  enabled: true
  servers:
    - jira-redux
  inline_servers:
    jira-redux:
      type: sse
      url: http://localhost:8080/events
      headers:
        Authorization: "Bearer ${JIRA_TOKEN}"
      timeout: 60000
      health_check:
        interval: 30
        max_failures: 5
```

### 3. Multiple Servers

```yaml
mcp:
  enabled: true
  servers:
    - figma-tools
    - jira-redux
    - github-tools
  inline_servers:
    figma-tools:
      command: npx
      args: ["-y", "figma-developer-mcp", "--stdio"]
      env:
        FIGMA_API_KEY: "${FIGMA_API_KEY}"
    
    jira-redux:
      type: sse
      url: http://localhost:8080/events
      headers:
        Authorization: "Bearer ${JIRA_TOKEN}"
    
    github-tools:
      command: npx
      args: ["-y", "@modelcontextprotocol/server-github", "--stdio"]
      env:
        GITHUB_TOKEN: "${GITHUB_TOKEN}"
```

## Setting Up MCP Servers

### Prerequisites

1. **Node.js**: Required for most MCP servers
2. **API Keys**: Obtain necessary API keys for services (FIGMA, JIRA, etc.)
3. **Environment Variables**: Set up authentication tokens

### Step 1: Install MCP Server

For npm-based MCP servers:

```bash
# Global installation
npm install -g figma-developer-mcp

# Or use npx (recommended - no installation needed)
npx figma-developer-mcp --stdio
```

### Step 2: Configure Authentication

Set environment variables:

```bash
# For FIGMA
export FIGMA_API_KEY="your-figma-api-key"

# For JIRA
export JIRA_TOKEN="your-jira-token"
export JIRA_URL="https://your-domain.atlassian.net"

# For GitHub
export GITHUB_TOKEN="ghp_your_github_token"
```

### Step 3: Update Agent Configuration

Add MCP settings to your `config.yaml`:

```yaml
# Enable MCP
mcp:
  enabled: true
  servers:
    - your-server-name
  inline_servers:
    your-server-name:
      # Server configuration here
```

### Step 4: Verify Connection

Test the MCP connection:

```bash
# Start the agent
bedrock-agent test

# List available tools (should include MCP tools)
bedrock-agent tools
```

## Transport Types

### Stdio Transport

Used for process-based MCP servers:

**Characteristics**:
- Direct process communication
- Low latency
- Suitable for local tools
- Process lifecycle managed by agent

**Configuration**:
```yaml
server-name:
  command: /path/to/executable
  args: ["--stdio", "--other-args"]
  env:
    KEY: "value"
  timeout: 30000
```

### SSE Transport

Used for HTTP-based MCP servers:

**Characteristics**:
- Network-based communication
- Supports remote servers
- Real-time event streaming
- Automatic reconnection

**Configuration**:
```yaml
server-name:
  type: sse
  url: http://server:port/events
  headers:
    Authorization: "Bearer token"
  timeout: 60000
```

## Using MCP Tools

Once configured, MCP tools are automatically available:

### List Available Tools

```bash
bedrock-agent tools
```

Output includes both built-in and MCP tools:
```
Available tools:
- fs_read (built-in)
- fs_write (built-in)
- figma_get_file (MCP: figma-tools)
- jira_create_issue (MCP: jira-redux)
```

### Execute MCP Tools

Use MCP tools in your prompts:

```bash
# FIGMA example
bedrock-agent task --prompt "Get the design components from FIGMA file ABC123"

# JIRA example
bedrock-agent task --prompt "Create a JIRA issue for fixing the login bug"

# Combined example
bedrock-agent task --prompt "Check the FIGMA design and create corresponding JIRA tasks"
```

## Health Monitoring

MCP servers are monitored for health:

```yaml
health_check:
  interval: 60        # Check every 60 seconds
  max_failures: 3     # Restart after 3 failures
  timeout: 5000       # Health check timeout
```

### Health Check Process

1. Periodic health ping sent to server
2. If no response within timeout, marked as failure
3. After max_failures, server is restarted
4. Automatic recovery without manual intervention

## Troubleshooting

### Issue: MCP server not starting

**Check**:
```bash
# Test server directly
npx figma-developer-mcp --stdio

# Check logs
RUST_LOG=debug bedrock-agent test
```

### Issue: Tools not appearing

**Verify**:
1. MCP is enabled in config
2. Server is listed in servers array
3. Server configuration is correct
4. Authentication is set up

### Issue: SSE connection failing

**Debug**:
```bash
# Test SSE endpoint
curl -H "Authorization: Bearer $TOKEN" \
     http://localhost:8080/events
```

### Issue: Tool execution errors

**Check**:
- API keys are valid
- Server has necessary permissions
- Network connectivity (for SSE)
- Process permissions (for stdio)

## Advanced Configuration

### Custom MCP Servers

Create your own MCP server:

```javascript
// mcp-server.js
const { MCPServer } = require('@modelcontextprotocol/sdk');

const server = new MCPServer({
  name: 'custom-tools',
  version: '1.0.0',
  tools: [
    {
      name: 'custom_tool',
      description: 'My custom tool',
      parameters: { /* schema */ },
      handler: async (params) => {
        // Tool implementation
        return { result: 'success' };
      }
    }
  ]
});

server.start();
```

### Retry Configuration

Configure retry behavior:

```yaml
server-name:
  retry:
    max_attempts: 5
    initial_delay: 1000
    max_delay: 30000
    backoff: "exponential"  # or "linear", "fixed"
```

### Environment Variables

Use environment variable substitution:

```yaml
server-name:
  env:
    API_KEY: "${MY_API_KEY}"
    API_URL: "${API_URL:-https://default.url}"
    DEBUG: "${DEBUG:-false}"
```

## Security Considerations

1. **API Key Management**:
   - Never commit API keys to version control
   - Use environment variables or secret managers
   - Rotate keys regularly

2. **Network Security**:
   - Use HTTPS for SSE connections
   - Validate SSL certificates
   - Consider VPN for sensitive data

3. **Process Security**:
   - Run MCP servers with minimal permissions
   - Sandbox process execution
   - Monitor resource usage

## Performance Tips

1. **Connection Pooling**: MCP connections are reused
2. **Timeout Configuration**: Adjust based on tool complexity
3. **Health Check Intervals**: Balance between detection speed and overhead
4. **Concurrent Tools**: Multiple MCP servers can run simultaneously

## Example Workflows

### Design to Development (FIGMA + JIRA)

```bash
bedrock-agent task --prompt "
1. Get the latest designs from FIGMA file XYZ
2. Analyze the components that need implementation
3. Create JIRA tasks for each component
4. Add design links to each JIRA issue
"
```

### Code Review with GitHub Tools

```bash
bedrock-agent task --prompt "
Review the open PRs in the repository,
summarize the changes, and update the PR descriptions
"
```

## Supported MCP Servers

Known working MCP servers:

| Server | Transport | Purpose |
|--------|-----------|---------|
| FIGMA | Stdio | Design operations |
| JIRA | SSE | Issue tracking |
| GitHub | Stdio | Repository management |
| Slack | SSE | Communication |
| Database | Stdio | Data operations |
| FileSystem | Stdio | Extended file operations |

## Next Steps

1. **Explore Available Servers**: Check [MCP Registry](https://modelcontextprotocol.io/servers)
2. **Create Custom Tools**: Build your own MCP servers
3. **Integrate with Workflows**: Combine multiple MCP servers
4. **Monitor Performance**: Track tool usage and optimize

---

*MCP integration enables powerful tool extensions beyond the built-in capabilities, allowing the Bedrock CLI Agent to interact with any service that provides an MCP server.*
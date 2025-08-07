# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-mcp** - Model Context Protocol (MCP) integration for external tool discovery and registration. Enables connection to MCP servers that provide additional tools beyond the built-in set. *Currently planned but not yet implemented.*

## Planned Architecture

### Core Components
- **McpClient**: Protocol client for MCP communication
- **McpManager**: Manages multiple MCP server connections
- **McpToolAdapter**: Adapts MCP tools to bedrock-tools `Tool` trait
- **Transport**: Abstractions for stdio and SSE connections

### MCP Protocol Support
```rust
// Planned message types
pub enum McpMessage {
    Initialize { capabilities: Capabilities },
    ToolList { tools: Vec<McpTool> },
    ToolCall { id: String, tool: String, arguments: Value },
    ToolResult { id: String, result: Value },
    Error { code: i32, message: String },
}
```

## Development Guidelines

### Adding MCP Support (TODO)
1. Implement transport layer (stdio/SSE)
2. Create protocol message handling
3. Build tool adapter for `Tool` trait
4. Integrate with `ToolRegistry`
5. Add configuration support

### Planned Transport Types
```rust
pub enum Transport {
    Stdio(StdioTransport),      // Process communication
    Sse(SseTransport),          // Server-sent events
    WebSocket(WsTransport),     // Future: WebSocket support
}
```

### Tool Adapter Pattern
```rust
// Convert MCP tool to bedrock Tool
pub struct McpToolAdapter {
    mcp_tool: McpTool,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.mcp_tool.name
    }
    
    async fn execute(&self, args: Value) -> Result<String> {
        // Send to MCP server
        let result = self.client.call_tool(
            &self.mcp_tool.name,
            args
        ).await?;
        
        // Return result
        Ok(result.to_string())
    }
}
```

## Planned Configuration

```yaml
mcp:
  servers:
    - name: "filesystem-tools"
      transport: "stdio"
      command: "mcp-filesystem"
      args: ["--workspace", "${WORKSPACE_DIR}"]
    
    - name: "database-tools"
      transport: "sse"
      url: "http://localhost:3000/sse"
      headers:
        Authorization: "Bearer ${MCP_TOKEN}"
```

## Implementation Roadmap

### Phase 1: Core Protocol
- [ ] Define MCP message types
- [ ] Implement JSON-RPC handling
- [ ] Create transport abstraction
- [ ] Build stdio transport

### Phase 2: Tool Integration
- [ ] Tool discovery mechanism
- [ ] Tool adapter implementation
- [ ] Registry integration
- [ ] Error handling

### Phase 3: Advanced Features
- [ ] SSE transport support
- [ ] Connection pooling
- [ ] Automatic reconnection
- [ ] Tool caching

## MCP Protocol Basics

### Initialization Flow
1. Connect to MCP server
2. Send `initialize` request
3. Receive server capabilities
4. Request tool list
5. Register tools with adapter

### Tool Execution Flow
1. Receive tool call from agent
2. Forward to MCP server
3. Await response
4. Handle errors/timeouts
5. Return result to agent

## Error Handling Strategy

```rust
pub enum McpError {
    ConnectionFailed(String),
    ProtocolError(String),
    ToolNotFound(String),
    ExecutionFailed { tool: String, error: String },
    Timeout(Duration),
}
```

## Testing Approach

### Mock MCP Server
Create test server for development:
```rust
pub struct MockMcpServer {
    tools: HashMap<String, MockTool>,
}

impl MockMcpServer {
    pub async fn start(&self) -> Result<()> {
        // Start stdio or HTTP server
    }
}
```

### Integration Tests
```bash
cargo test -p bedrock-mcp --features integration
```

## Security Considerations

- Validate tool inputs before forwarding
- Sandbox MCP server processes
- Implement timeout for all operations
- Verify server certificates for HTTPS
- Rate limit tool executions

## Performance Goals

- Tool discovery: < 100ms
- Tool execution overhead: < 10ms
- Connection pooling for multiple servers
- Async/concurrent tool calls
- Response caching where appropriate

## Dependencies (Planned)

- `tokio`: Async runtime
- `serde_json`: Protocol messages
- `reqwest`: HTTP/SSE transport
- `tokio-tungstenite`: WebSocket support (future)

## Notes for Implementation

When implementing this crate:
1. Start with stdio transport (simplest)
2. Focus on robust error handling
3. Make tool discovery automatic
4. Ensure backward compatibility
5. Document MCP server requirements

## Current Status

⚠️ **This crate is planned but not yet implemented**

The crate structure exists but needs:
- Protocol implementation
- Transport layer
- Tool adapter
- Integration with main agent
- Configuration support
- Documentation and examples
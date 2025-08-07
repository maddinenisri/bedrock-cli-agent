# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-client** - AWS Bedrock Runtime integration with streaming support, tool execution, and conversation management. Provides the core communication layer between the agent and AWS Bedrock's Converse API.

## Key Components

### Main Client (`src/lib.rs`)
- **BedrockClient**: Wrapper around AWS SDK client with configuration
- **converse()**: Non-streaming API for simple requests
- **converse_stream()**: Streaming API with real-time output and tool support
- **execute_tools()**: Orchestrates tool execution within conversations

### Streaming Handler (`src/streaming.rs`)
- Processes stream events: `ContentBlockDelta`, `ContentBlockStart`, `ContentBlockStop`, `MessageStop`, `Metadata`
- Real-time display with newline filtering (max 2 consecutive)
- Tool usage detection and visual feedback (`🛠️` indicators)
- Response reconstruction from stream events

### Display Utilities (`src/ui.rs`)
- Terminal output formatting
- Tool execution status display
- Stream event visualization

## AWS Integration Details

### Authentication
```rust
// Supports full AWS credential chain
let config = aws_config::from_env()
    .profile_name(&aws_settings.profile)  // Optional profile
    .region(region)
    .load()
    .await;
```

### Message Conversion
- **JSON ↔ Document**: Bidirectional conversion between `serde_json::Value` and AWS `Document`
- **Tool Specs**: Convert `ToolDefinition` to AWS `ToolSpecification` with JSON schemas
- **Content Blocks**: Handle text and tool use blocks in responses

## Development Guidelines

### Adding Features
1. **New Stream Events**: Update match statement in `converse_stream()`
2. **Tool Support**: Modify `execute_tools()` and tool conversion functions
3. **Error Cases**: Add specific variants to `BedrockError` in core crate

### Testing Commands
```bash
cargo test -p bedrock-client          # Run tests (when added)
cargo build -p bedrock-client         # Build only this crate
RUST_LOG=bedrock_client=trace cargo run -- test  # Debug logging
```

## Important Implementation Details

### Streaming Response Flow
1. Initialize accumulators (text, tool_input, tool_name, tool_id)
2. Process events in real-time with stdout flushing
3. Display tool usage indicators when detected
4. Reconstruct complete response for return value
5. Handle metadata for token statistics

### Tool Execution Pattern
```rust
// 1. Check for tool use in response
if response.has_tool_use() {
    // 2. Extract tool calls
    let tool_uses = response.get_tool_uses();
    
    // 3. Execute each tool
    for tool_use in tool_uses {
        let result = registry.execute_tool(...).await;
        // 4. Convert result to ToolResultBlock
    }
}
```

### Error Handling Strategy
- Tool errors are isolated (one tool failure doesn't fail conversation)
- Stream errors wrapped in `BedrockError::Unknown`
- AWS SDK errors converted via `map_err`
- Missing tools handled gracefully with error status

## Configuration

Uses `AgentConfig` with:
- Model selection (e.g., `anthropic.claude-3-5-sonnet-20241022-v2:0`)
- Max tokens, temperature settings
- System prompts
- Tool configuration

## Performance Considerations

- **Streaming**: Reduces memory usage for large responses
- **Arc<Config>**: Efficient config sharing across async tasks
- **LRU Cache**: Dependency included but not yet implemented
- **Immediate Flush**: Real-time output during streaming

## Areas Needing Work

1. **Retry Logic**: Add exponential backoff for transient failures
2. **Rate Limiting**: Implement using existing dependency
3. **Response Caching**: Activate LRU cache for repeated queries
4. **Unit Tests**: Add comprehensive test coverage
5. **Metrics**: Enhance tracing with performance metrics

## Common Patterns

### Making a Streaming Request
```rust
let response = client.converse_stream(
    messages,
    tools,
    system_prompts,
    max_tokens,
    temperature,
    stop_sequences
).await?;
```

### Processing Tool Results
```rust
let tool_result = ToolResultBlock::builder()
    .tool_use_id(tool_use_id)
    .content(ToolResultContentBlock::Text(output))
    .status(if success { Success } else { Error })
    .build()?;
```

### Handling Stream Events
```rust
match event {
    ContentBlockDelta(delta) => {
        // Accumulate text or tool input
    },
    ContentBlockStop(_) => {
        // Finalize current block
    },
    MessageStop(_) => {
        // Complete response processing
    },
    _ => {} // Handle other events
}
```

## Dependencies to Note

- `aws-sdk-bedrockruntime`: Core AWS integration
- `async-stream`: Ergonomic async streaming
- `tokio`: Async runtime
- `lru`: Caching (not yet used)
- `governor`: Rate limiting (not yet used)
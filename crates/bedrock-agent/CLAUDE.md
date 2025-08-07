# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-agent** - High-level agent orchestration that combines all components into a cohesive system. Implements the `Agent` trait from bedrock-core and provides chat interfaces with streaming support.

## Core Architecture

### Agent Struct
```rust
pub struct Agent {
    config: Arc<AgentConfig>,
    client: Arc<BedrockClient>,
    tool_registry: Arc<ToolRegistry>,
    task_executor: TaskExecutor,
}
```

Implements `bedrock_core::Agent` trait with:
- `execute_task()`: Process tasks with tool support
- `cancel_task()`: Task cancellation (TODO)
- `get_task_status()`: Status queries (TODO)

## Key Features

### Chat Interface
```rust
// Non-streaming chat
pub async fn chat(&self, messages, tools) -> Result<StreamResult>

// Streaming chat with callback
pub async fn chat_stream<F>(&self, messages, tools, callback: F) -> Result<StreamResult>
where F: Fn(&str) + Send + 'static
```

### Tool Integration
- Automatic tool registration from config
- Tool execution loop management
- Conversation state handling

## Development Guidelines

### Creating an Agent
```rust
// From configuration file
let agent = Agent::from_config("config.yaml").await?;

// From existing config
let config = AgentConfig::from_file("config.yaml")?;
let agent = Agent::new(config).await?;
```

### Testing Commands
```bash
cargo test -p bedrock-agent              # All tests
cargo run --example simple_task          # Example usage
```

## Important Implementation Details

### Agent Initialization Flow
1. Load and validate configuration
2. Create AWS Bedrock client
3. Initialize tool registry with defaults
4. Setup task executor
5. Return configured agent

### Chat Implementation
```rust
// Build initial message
let mut messages = vec![
    Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text(prompt))
        .build()?
];

// Execute with tools
let response = self.execute_with_tools(
    messages,
    tools,
    system_prompts,
    max_tokens,
    temperature
).await?;
```

### Streaming Architecture
- Uses `tokio_stream` for async streaming
- Callback function for real-time output
- Accumulates full response for return value
- Tracks tokens and costs during stream

### Tool Execution Loop
```rust
const MAX_ITERATIONS: usize = 10;

async fn execute_with_tools(&self, ...) -> Result<StreamResult> {
    for iteration in 0..MAX_ITERATIONS {
        // Get response from model
        let response = client.converse(...).await?;
        
        // Check for tool calls
        if !response.has_tool_use() {
            break;
        }
        
        // Execute tools
        let tool_results = client.execute_tools(...).await?;
        
        // Add results to conversation
        messages.push(tool_result_message);
    }
}
```

## Common Patterns

### Basic Task Execution
```rust
let task = Task::new("Analyze this file");
let result = agent.execute_task(task).await?;
println!("Result: {}", result.summary);
```

### Streaming Chat
```rust
let callback = |chunk: &str| {
    print!("{}", chunk);
    std::io::stdout().flush().unwrap();
};

let result = agent.chat_stream(
    "Tell me a story",
    None,  // Optional tools
    callback
).await?;
```

### With Custom Tools
```rust
let tools = vec![
    ToolDefinition {
        name: "custom_tool".to_string(),
        description: "My custom tool".to_string(),
        input_schema: json!({"type": "object"}),
    }
];

let result = agent.chat("Use the tool", Some(tools)).await?;
```

## Integration Points

### Component Coordination
- **Config**: Drives all component settings
- **Client**: Handles AWS Bedrock communication
- **Tools**: Provides execution capabilities
- **Task Executor**: Manages task queue and processing

### Error Propagation
All errors bubble up as `BedrockError`:
- Configuration errors
- AWS API errors
- Tool execution failures
- Task processing issues

## Architecture Notes

### Why Arc Everything?
Components need to be shared across async tasks:
- Config accessed by multiple components
- Client used concurrently
- Tool registry shared between tasks
- Thread-safe access required

### Streaming vs Non-Streaming
- **Streaming**: Real-time output, callback-based
- **Non-streaming**: Simpler API, full response
- Both return same `StreamResult` type

### Task Executor Integration
Agent delegates task management to TaskExecutor:
- Priority queuing
- Concurrent execution
- Result persistence

## Performance Considerations

- Configuration cached in Arc
- Tool registry pre-built at startup
- Streaming reduces memory for large responses
- Concurrent task processing support

## Dependencies to Note

- All other bedrock crates
- `aws-sdk-bedrockruntime`: AWS types
- `tokio` + `tokio-stream`: Async streaming
- `async-stream`: Stream macros
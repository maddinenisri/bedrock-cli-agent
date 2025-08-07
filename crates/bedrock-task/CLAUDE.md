# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-task** - Task queue management and execution orchestration with conversation state handling, tool iteration loops, and result persistence. Manages the core execution flow of agent tasks.

## Key Components

### TaskExecutor
Main orchestration engine:
- Priority-based task queue (`BinaryHeap`)
- Tool iteration loop management
- Conversation state tracking
- Token and cost calculation

### Task Types
- **QueuedTask**: Wrapper with priority and timestamp
- **Priority**: High/Normal/Low ordering
- **Task**: Core task from bedrock-core

## Core Execution Flow

### Tool Iteration Loop
```rust
// Maximum 10 iterations to prevent infinite loops
const MAX_TOOL_ITERATIONS: usize = 10;

// Tool execution loop pattern:
1. Send message to model
2. Check if response has tool calls
3. Execute tools and collect results
4. Add tool results as User message
5. Repeat until no more tool calls or max iterations
```

### Message Building
```rust
// Tool results MUST be User messages with ContentBlock::ToolResult
let tool_message = Message::builder()
    .role(ConversationRole::User)
    .set_content(Some(
        tool_results.into_iter()
            .map(ContentBlock::ToolResult)
            .collect()
    ))
    .build()?;
```

## Development Guidelines

### Task Processing
1. Tasks queued with priority
2. Executor processes based on priority order
3. Conversation maintained across tool iterations
4. Results include full conversation history

### Testing Commands
```bash
cargo test -p bedrock-task                # All tests
cargo test -p bedrock-task test_priority  # Priority queue tests
```

## Important Implementation Details

### Priority Queue Ordering
```rust
impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older tasks
        self.priority.cmp(&other.priority)
            .then_with(|| other.timestamp.cmp(&self.timestamp))
    }
}
```

### Conversation Management
- Messages stored as `Vec<Message>` from AWS SDK
- Tool results added as User messages
- System prompts included in first request only
- Full conversation preserved in TaskResult

### Token and Cost Tracking
```rust
// Accumulate across iterations
let mut total_input_tokens = 0;
let mut total_output_tokens = 0;

for iteration in 0..MAX_TOOL_ITERATIONS {
    // Get response with stats
    let response = client.converse(...).await?;
    
    // Accumulate tokens
    if let Some(usage) = response.usage() {
        total_input_tokens += usage.input_tokens as usize;
        total_output_tokens += usage.output_tokens as usize;
    }
}
```

### Tool Execution
```rust
// Execute tools from response
let tool_results = client.execute_tools(
    &response,
    &tool_registry
).await?;

// Check if all succeeded
let has_errors = tool_results.iter()
    .any(|r| matches!(r.status(), Some(Error)));
```

## Common Patterns

### Creating and Queueing Tasks
```rust
let task = Task::new("prompt").with_context("context");
executor.queue_task(task, Priority::Normal).await;
```

### Processing Tasks
```rust
let result = executor.process_next().await?;
match result.status {
    TaskStatus::Completed => { /* Success */ },
    TaskStatus::Failed => { /* Handle error */ },
    _ => { /* Other states */ }
}
```

### Conversation Serialization
```rust
// Convert AWS Messages to JSON for persistence
let conversation_json: Vec<Value> = conversation
    .iter()
    .map(|msg| /* custom serialization */)
    .collect();
```

## Error Handling

Common error scenarios:
- Tool execution failures (captured in results)
- Max iterations exceeded (prevents infinite loops)
- AWS API errors (rate limits, auth)
- Serialization issues (Message to JSON)

## Architecture Notes

### Why User Messages for Tools
AWS Bedrock requires tool results as User messages with `ContentBlock::ToolResult`. This is different from Assistant messages with tool use.

### Iteration Limits
Hard limit of 10 iterations prevents:
- Infinite tool loops
- Excessive token usage
- Runaway costs

### Priority System
Enables:
- Urgent task handling
- Background processing
- Fair scheduling with timestamps

## Dependencies to Note

- `aws-sdk-bedrockruntime`: Message types
- `bedrock-client`: Model communication
- `bedrock-tools`: Tool execution
- `tokio`: Async runtime
- `uuid`: Task identification
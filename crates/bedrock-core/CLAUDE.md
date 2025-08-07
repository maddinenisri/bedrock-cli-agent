# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-core** - Foundation types and traits for the entire Bedrock CLI Agent system. Defines core abstractions without implementation details, maintaining AWS-agnostic design while providing essential types for task management, error handling, and agent contracts.

## Key Types and Their Usage

### Task Management
- **Task**: Unit of work with UUID, prompt, context, and timestamps. Use builder pattern: `Task::new("prompt").with_context("context")`
- **TaskResult**: Comprehensive execution outcome with conversation history, token stats, and cost details
- **TaskStatus**: Enum for task lifecycle (`Pending`, `Running`, `Completed`, `Failed`, `Cancelled`)
- **StreamResult**: Simplified result for streaming responses with text and metrics

### Error Handling
- **BedrockError**: Comprehensive error enum with specific variants for each failure type
- **Result<T>**: Type alias for `std::result::Result<T, BedrockError>`
- Use `#[from]` conversions for automatic error propagation
- Always provide context in error messages

### Core Trait
- **Agent**: Async trait defining agent contract with `execute_task`, `cancel_task`, `get_task_status`
- Requires `Send + Sync` for thread safety
- Uses `async_trait` macro for async methods

## Development Guidelines

### Adding New Types
1. All public types must derive `Serialize`, `Deserialize`, `Debug`, `Clone`
2. Use builder pattern for complex types
3. Implement `Default` where zero-values make sense
4. Add documentation comments for all public items

### Error Handling
- Add new error variants to `BedrockError` for specific failure cases
- Use `thiserror` attributes for automatic Display implementation
- Include relevant context (e.g., tool name, task ID) in error variants
- Avoid generic error messages

### Testing
```bash
cargo test -p bedrock-core           # Run all tests
cargo test -p bedrock-core test_name # Run specific test
```

## Important Implementation Details

- **Conversation Storage**: `TaskResult` stores conversation as `Vec<serde_json::Value>` to avoid AWS SDK serialization issues
- **UUID Generation**: Always use `Uuid::new_v4()` for task IDs
- **Timestamps**: Use `Utc::now()` from chrono for all timestamps
- **Token Statistics**: Always initialize with `Default::default()` then update fields
- **Cost Calculation**: Store costs as `f64` with currency string (default "USD")

## Dependencies to Note

- `serde`/`serde_json`: All types must be serializable
- `uuid`: Task identification
- `chrono`: UTC timestamp handling
- `async-trait`: Required for Agent trait
- `thiserror`: Error derive macros

## Common Patterns

### Creating Tasks
```rust
let task = Task::new("Analyze this code")
    .with_context("Focus on performance");
```

### Error Propagation
```rust
// Use ? operator with automatic conversion
let result = some_operation()?;

// Or create specific errors
return Err(BedrockError::ToolError {
    tool: "grep".to_string(),
    message: "Pattern not found".to_string(),
});
```

### Working with TaskResult
```rust
let mut result = TaskResult::default();
result.task_id = task.task_id;
result.status = TaskStatus::Running;
result.started_at = Some(Utc::now());
// Update as execution progresses
```

## Architecture Notes

This crate is the foundation - keep it:
- **Minimal**: Only core types, no implementation logic
- **AWS-agnostic**: No AWS SDK dependencies here
- **Stable**: Changes here affect entire codebase
- **Well-documented**: This is the contract other crates depend on
# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-tools** - Extensible tool system with built-in file operations, search capabilities, and command execution. All tools implement workspace sandboxing and security constraints.

## Core Architecture

### Tool Trait
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;  // JSON schema
    async fn execute(&self, args: Value) -> Result<String>;
}
```

### Tool Registry
Thread-safe registry for tool management:
- Registration with `register_tool()`
- Lookup with `get_tool()`
- Listing with `list_tools()`
- Uses `Arc<RwLock<HashMap>>` for concurrent access

## Built-in Tools

### File System Tools
- **fs_read**: Read files (max 10MB default)
- **fs_write**: Write content to files
- **fs_list**: List directory contents

Security features:
- Workspace directory sandboxing
- Path canonicalization prevents traversal
- File size limits
- UTF-8 validation

### Search Tools
- **grep**: Pattern search using ripgrep
- **find**: Find files by name patterns
- **ripgrep**: Advanced search with regex

### Execution Tool
- **execute_bash**: Run shell commands
  - Cross-platform (bash/cmd)
  - Timeout support (30s default)
  - Output size limits
  - Shell detection for complex commands

## Development Guidelines

### Adding New Tools
1. Create struct implementing `Tool` trait
2. Define JSON schema for arguments
3. Implement async `execute()` method
4. Register in `ToolRegistry::new_with_defaults()`

Example:
```rust
#[derive(Debug)]
pub struct MyTool {
    workspace_dir: PathBuf,
}

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    
    fn description(&self) -> &str { 
        "Description of my tool"
    }
    
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "param": {"type": "string"}
            },
            "required": ["param"]
        })
    }
    
    async fn execute(&self, args: Value) -> Result<String> {
        // Implementation
    }
}
```

### Testing Commands
```bash
cargo test -p bedrock-tools              # All tests
cargo test -p bedrock-tools test_grep    # Specific test
```

## Security Patterns

### Path Validation
```rust
fn validate_path(&self, path: &Path) -> Result<PathBuf> {
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        self.workspace_dir.join(path)
    };
    
    let canonical = full_path.canonicalize()?;
    
    if !canonical.starts_with(&self.workspace_dir) {
        return Err(BedrockError::ToolError {
            tool: self.name().to_string(),
            message: "Path outside workspace".to_string(),
        });
    }
    
    Ok(canonical)
}
```

### Command Execution Safety
- Shell detection for pipes/redirects
- Timeout enforcement
- Output size limits
- Cross-platform compatibility

## Common Patterns

### Tool Registration
```rust
let registry = ToolRegistry::new_with_defaults(workspace_dir);
registry.register_tool("custom", Arc::new(CustomTool::new()));
```

### Tool Execution
```rust
let args = json!({
    "path": "file.txt"
});
let result = registry.execute_tool("fs_read", args).await?;
```

### JSON Schema Definition
```rust
fn schema(&self) -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "File path"
            }
        },
        "required": ["path"]
    })
}
```

## Important Implementation Details

- **Workspace Isolation**: All paths resolved relative to workspace
- **Async Execution**: All tools use async for I/O operations
- **Error Context**: Include tool name in all errors
- **Cross-platform**: Handle Windows vs Unix differences
- **UTF-8 Only**: File operations assume UTF-8 encoding

## Tool Schemas

### File Operations
```json
{
  "fs_read": {
    "path": "string (required)"
  },
  "fs_write": {
    "path": "string (required)",
    "content": "string (required)"
  },
  "fs_list": {
    "path": "string (optional, default: .)"
  }
}
```

### Search Operations
```json
{
  "grep": {
    "pattern": "string (required)",
    "path": "string (optional)"
  },
  "find": {
    "pattern": "string (required)",
    "path": "string (optional)"
  }
}
```

### Command Execution
```json
{
  "execute_bash": {
    "command": "string (required)"
  }
}
```

## Dependencies to Note

- `async-trait`: Async trait support
- `tokio`: Async runtime and process spawning
- `serde_json`: Argument parsing
- `bedrock-core`: Error types
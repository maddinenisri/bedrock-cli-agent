# bedrock-conversation

Conversation management and persistence for the Bedrock CLI Agent.

## Features

- 📝 JSONL-based conversation storage
- 🔄 Resume conversations with full history
- 💾 Automatic persistence to disk
- 🏷️ Workspace-based organization
- 📊 Token usage and cost tracking
- 🎯 Task result tracking
- 📈 Conversation statistics

## Usage

### Creating a Conversation Manager

```rust
use bedrock_conversation::{ConversationManager, ConversationStorage};

// Create a new conversation manager
let mut manager = ConversationManager::new()?;

// Start a new conversation
let metadata = manager.start_conversation(
    "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
    Some("You are a helpful assistant".to_string())
)?;

// Resume an existing conversation
let messages = manager.resume_conversation(conversation_id)?;
```

### Storing Messages

```rust
use bedrock_conversation::MessageEntry;

// Add user message
manager.add_user_message("Hello, how are you?".to_string())?;

// Add assistant message with token tracking
manager.add_assistant_message(
    "I'm doing well, thank you!".to_string(),
    Some(TokenUsageStats {
        input_tokens: 10,
        output_tokens: 15,
        total_tokens: 25,
        total_cost: Some(0.0002),
    })
)?;

// Add tool message
manager.add_tool_message(
    "fs_read".to_string(),
    "tool_use_123".to_string(),
    json!({"content": "File contents here"})
)?;
```

### Conversation Storage

```rust
use bedrock_conversation::ConversationStorage;

let storage = ConversationStorage::new()?;

// List all conversations
let conversations = storage.list_conversations()?;

// Load conversation metadata
let metadata = storage.load_metadata(&conversation_id)?;

// Read conversation messages
let messages = storage.read_messages(&conversation_id)?;

// Delete a conversation
storage.delete_conversation(&conversation_id)?;

// Export conversation
storage.export_conversation(&conversation_id, &output_path)?;
```

## Storage Format

### Directory Structure

```
~/.bedrock-agent/conversations/
├── <workspace-hash>-<workspace-name>/
│   ├── index.json                    # Conversation index
│   └── <conversation-id>/
│       ├── metadata.json             # Conversation metadata
│       └── messages.jsonl            # Message history (JSONL)
```

### Message Format (JSONL)

Each line in `messages.jsonl` is a JSON object:

```json
{
  "timestamp": "2024-01-08T10:30:00Z",
  "role": "user|assistant|tool",
  "content": "message content or JSON value",
  "tool_name": "optional tool name",
  "tool_use_id": "optional tool use ID",
  "tokens": {
    "input_tokens": 100,
    "output_tokens": 200,
    "total_tokens": 300,
    "total_cost": 0.005
  }
}
```

### Metadata Format

```json
{
  "id": "uuid-v4",
  "model_id": "anthropic.claude-3-5-sonnet-20241022-v2:0",
  "system_prompt": "optional system prompt",
  "created_at": "2024-01-08T10:00:00Z",
  "updated_at": "2024-01-08T10:30:00Z",
  "working_directory": "/path/to/workspace",
  "message_count": 10,
  "has_tasks": true,
  "task_count": 2,
  "completed_tasks": 1,
  "failed_tasks": 0,
  "token_usage": {
    "input_tokens": 1000,
    "output_tokens": 2000,
    "total_tokens": 3000,
    "total_cost": 0.05
  }
}
```

## Workspace Organization

Conversations are organized by workspace using a hash of the current working directory:

- Hash is first 8 characters of SHA-256
- Directory name includes both hash and workspace name
- Allows multiple projects to maintain separate conversations
- Prevents collision while maintaining readability

Example: `351a96ed-bedrock-cli-agent/`

## Integration with CLI

The conversation management is integrated with the CLI commands:

```bash
# List conversations
bedrock-agent list

# Resume conversation
bedrock-agent conversation <id>

# Generate summary
bedrock-agent conversation <id> --summary

# Export conversation
bedrock-agent conversation <id> --export backup.json

# Import conversation
bedrock-agent import backup.json
```

## Error Handling

All operations return `Result<T>` with appropriate error context:

- File I/O errors
- JSON parsing errors
- Invalid conversation IDs
- Missing metadata

## Thread Safety

The `ConversationStorage` is designed to be used from a single thread. For concurrent access, use appropriate synchronization mechanisms like `Arc<Mutex<>>`.
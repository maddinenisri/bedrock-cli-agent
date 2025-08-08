use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::Tool;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoItem {
    id: usize,
    content: String,
    completed: bool,
}

#[derive(Clone, Default)]
pub struct TodoTool {
    todos: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodoTool {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Simple in-memory todo list management"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "complete"]
                },
                "item": { "type": "string" },
                "id": { "type": "integer" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let action =
            args.get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| BedrockError::ToolError {
                    tool: self.name().to_string(),
                    message: "action is required".to_string(),
                })?;

        match action {
            "add" => {
                let item = args.get("item").and_then(|v| v.as_str()).ok_or_else(|| {
                    BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "item is required".to_string(),
                    }
                })?;
                let mut todos = self.todos.lock().await;
                let id = todos.len() + 1;
                todos.push(TodoItem {
                    id,
                    content: item.to_string(),
                    completed: false,
                });
                Ok(json!({"id": id, "status": "added"}))
            }
            "list" => {
                let todos = self.todos.lock().await;
                Ok(json!({"todos": &*todos}))
            }
            "complete" => {
                let id = args.get("id").and_then(|v| v.as_u64()).ok_or_else(|| {
                    BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "id is required".to_string(),
                    }
                })? as usize;
                let mut todos = self.todos.lock().await;
                if let Some(todo) = todos.iter_mut().find(|t| t.id == id) {
                    todo.completed = true;
                    Ok(json!({"id": id, "status": "completed"}))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("todo id {} not found", id),
                    })
                }
            }
            _ => Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: "invalid action".to_string(),
            }),
        }
    }
}

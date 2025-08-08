use async_trait::async_trait;
use bedrock_core::{BedrockError, Result, TodoItem, TodoStatus};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::Tool;

pub struct TodoTool;

impl TodoTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Create todo items from descriptions"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            },
            "required": ["items"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let items: Vec<String> =
            serde_json::from_value(args["items"].clone()).map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: e.to_string(),
            })?;
        let todos: Vec<Value> = items
            .into_iter()
            .map(|desc| {
                let item = TodoItem {
                    id: Uuid::new_v4(),
                    description: desc,
                    status: TodoStatus::Pending,
                    created_at: chrono::Utc::now(),
                };
                json!({
                    "id": item.id.to_string(),
                    "description": item.description,
                    "status": "Pending"
                })
            })
            .collect();
        Ok(json!({"items": todos}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_todo_items() {
        let tool = TodoTool::new();
        let input = json!({"items": ["a task"]});
        let result = tool.execute(input).await.unwrap();
        assert!(result["items"].is_array());
        assert_eq!(result["items"].as_array().unwrap().len(), 1);
        let first = &result["items"][0];
        assert_eq!(first["description"], "a task");
        assert_eq!(first["status"], "Pending");
    }
}

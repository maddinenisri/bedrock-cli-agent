use crate::Tool;
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: Uuid,
    pub description: String,
    pub status: TodoStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TodoStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoInput {
    action: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    todo_id: Option<String>,
}

pub struct TodoManageTool {
    todos: Arc<Mutex<HashMap<Uuid, TodoItem>>>,
}

impl TodoManageTool {
    pub fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn format_todo_list(&self) -> String {
        let todos = self.todos.lock().unwrap();
        
        if todos.is_empty() {
            return "No todos yet. Create one using action: 'create'".to_string();
        }
        
        let mut output = String::from("\n📋 Current Todo List:\n");
        output.push_str("════════════════════════════════════════\n");
        
        // Group by status
        let mut pending = Vec::new();
        let mut in_progress = Vec::new();
        let mut completed = Vec::new();
        
        for todo in todos.values() {
            match todo.status {
                TodoStatus::Pending => pending.push(todo),
                TodoStatus::InProgress => in_progress.push(todo),
                TodoStatus::Completed => completed.push(todo),
            }
        }
        
        // Display in progress first
        if !in_progress.is_empty() {
            output.push_str("\n🔄 In Progress:\n");
            for todo in &in_progress {
                output.push_str(&format!("  ⏺ {}\n", todo.description));
            }
        }
        
        // Then pending
        if !pending.is_empty() {
            output.push_str("\n📝 Pending:\n");
            for todo in &pending {
                output.push_str(&format!("  ○ {}\n", todo.description));
            }
        }
        
        // Then completed
        if !completed.is_empty() {
            output.push_str("\n✅ Completed:\n");
            for todo in &completed {
                output.push_str(&format!("  ✓ {}\n", todo.description));
            }
        }
        
        // Progress summary
        let total = todos.len();
        let completed_count = completed.len();
        let in_progress_count = in_progress.len();
        
        output.push_str(&format!("\n📊 Progress: {}/{} completed, {} in progress\n", 
            completed_count, total, in_progress_count));
        
        output
    }
}

impl Default for TodoManageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoManageTool {
    fn name(&self) -> &str {
        "todo_manage"
    }

    fn description(&self) -> &str {
        "Manage a todo list to track task progress. Actions: create, start, complete, list"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "start", "complete", "list"],
                    "description": "The action to perform on the todo list"
                },
                "description": {
                    "type": "string",
                    "description": "Description of the todo item (required for 'create' action)"
                },
                "todo_id": {
                    "type": "string",
                    "description": "The ID of the todo to update (required for 'start' and 'complete' actions)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let input: TodoInput = serde_json::from_value(params)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid parameters: {}", e),
            })?;
        
        match input.action.as_str() {
            "create" => {
                let description = input.description
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Description is required for create action".to_string()
                    })?;
                
                let todo = TodoItem {
                    id: Uuid::new_v4(),
                    description: description.clone(),
                    status: TodoStatus::Pending,
                    created_at: Utc::now(),
                    started_at: None,
                    completed_at: None,
                };
                
                let todo_id = todo.id;
                self.todos.lock().unwrap().insert(todo_id, todo);
                
                let output = format!("✅ Created todo: {} (ID: {})\n{}", 
                    description, todo_id, self.format_todo_list());
                
                Ok(json!({
                    "success": true,
                    "todo_id": todo_id.to_string(),
                    "message": output
                }))
            }
            
            "start" => {
                let todo_id_str = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id is required for start action".to_string()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id_str)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Invalid todo_id: {}", todo_id_str)
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    todo.status = TodoStatus::InProgress;
                    todo.started_at = Some(Utc::now());
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("🔄 Started: {}\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Todo not found: {}", todo_id_str)
                    })
                }
            }
            
            "complete" => {
                let todo_id_str = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id is required for complete action".to_string()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id_str)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Invalid todo_id: {}", todo_id_str)
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    todo.status = TodoStatus::Completed;
                    todo.completed_at = Some(Utc::now());
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("✅ Completed: {}\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Todo not found: {}", todo_id_str)
                    })
                }
            }
            
            "list" => {
                let output = self.format_todo_list();
                
                Ok(json!({
                    "success": true,
                    "message": output
                }))
            }
            
            _ => Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Unknown action: {}. Valid actions are: create, start, complete, list", input.action)
            })
        }
    }
}
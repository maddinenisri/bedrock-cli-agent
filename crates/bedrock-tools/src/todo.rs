use crate::Tool;
use bedrock_core::{BedrockError, Result};
use bedrock_core::todo::{TodoItem, TodoStatus, TodoPriority, TodoBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoInput {
    action: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    todo_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    dependencies: Option<Vec<String>>,
}

pub struct TodoTool {
    todos: Arc<Mutex<HashMap<Uuid, TodoItem>>>,
}

impl TodoTool {
    pub fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    fn format_todo_list(&self) -> String {
        let todos = self.todos.lock().unwrap();
        
        if todos.is_empty() {
            return "No todos yet.".to_string();
        }
        
        let mut output = String::from("📋 Current Todos:\n");
        output.push_str("════════════════════════════════════════\n");
        
        // Group by status
        let mut pending = Vec::new();
        let mut in_progress = Vec::new();
        let mut completed = Vec::new();
        let mut failed = Vec::new();
        
        for todo in todos.values() {
            match todo.status {
                TodoStatus::Pending => pending.push(todo),
                TodoStatus::InProgress => in_progress.push(todo),
                TodoStatus::Completed => completed.push(todo),
                TodoStatus::Failed => failed.push(todo),
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
                let priority_icon = match todo.priority {
                    TodoPriority::High => "🔴",
                    TodoPriority::Medium => "🟡",
                    TodoPriority::Low => "🟢",
                };
                output.push_str(&format!("  {} {} {}\n", priority_icon, todo.description, 
                    if !todo.dependencies.is_empty() { 
                        format!("(depends on {} other todos)", todo.dependencies.len()) 
                    } else { 
                        String::new() 
                    }
                ));
            }
        }
        
        // Then completed
        if !completed.is_empty() {
            output.push_str("\n✅ Completed:\n");
            for todo in &completed {
                output.push_str(&format!("  ✓ {}\n", todo.description));
            }
        }
        
        // Finally failed
        if !failed.is_empty() {
            output.push_str("\n❌ Failed:\n");
            for todo in &failed {
                output.push_str(&format!("  ✗ {} - {}\n", 
                    todo.description,
                    todo.error.as_ref().map(|e| e.message.as_str()).unwrap_or("Unknown error")
                ));
            }
        }
        
        // Summary
        let total = todos.len();
        let completed_count = completed.len();
        let failed_count = failed.len();
        let in_progress_count = in_progress.len();
        
        output.push_str(&format!("\n📊 Progress: {}/{} completed, {} in progress, {} failed\n", 
            completed_count, total, in_progress_count, failed_count));
        
        output
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo_manage"
    }

    fn description(&self) -> &str {
        "Manage todos to track task progress. Actions: create, update, list, complete, fail"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "update", "list", "complete", "fail", "start"],
                    "description": "The action to perform"
                },
                "description": {
                    "type": "string",
                    "description": "The todo description (for create action)"
                },
                "todo_id": {
                    "type": "string",
                    "description": "The todo ID (for update/complete/fail/start actions)"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "failed"],
                    "description": "The new status (for update action)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["high", "medium", "low"],
                    "description": "The priority level (for create/update actions)"
                },
                "dependencies": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of todo IDs this depends on (for create action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let input: TodoInput = serde_json::from_value(params)?;
        
        match input.action.as_str() {
            "create" => {
                let description = input.description
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "description required for create".into()
                    })?;
                
                let mut builder = TodoBuilder::new(&description);
                
                if let Some(priority_str) = input.priority {
                    let priority = match priority_str.as_str() {
                        "high" => TodoPriority::High,
                        "medium" => TodoPriority::Medium,
                        "low" => TodoPriority::Low,
                        _ => TodoPriority::Medium,
                    };
                    builder = builder.priority(priority);
                }
                
                if let Some(deps) = input.dependencies {
                    let dep_ids: std::result::Result<Vec<Uuid>, _> = deps.iter()
                        .map(|s| Uuid::parse_str(s))
                        .collect();
                    
                    if let Ok(dep_ids) = dep_ids {
                        builder = builder.dependencies(dep_ids);
                    }
                }
                
                let todo = builder.build();
                let todo_id = todo.id;
                
                self.todos.lock().unwrap().insert(todo_id, todo);
                
                let output = format!("✅ Created todo: {} (ID: {})\n\n{}", 
                    description, todo_id, self.format_todo_list());
                
                Ok(json!({
                    "success": true,
                    "todo_id": todo_id.to_string(),
                    "message": output
                }))
            }
            
            "start" => {
                let todo_id = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id required for start".into()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Invalid todo_id".into()
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    todo.status = TodoStatus::InProgress;
                    todo.started_at = Some(Utc::now());
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("🔄 Started: {}\n\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Todo not found".into()
                    })
                }
            }
            
            "complete" => {
                let todo_id = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id required for complete".into()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Invalid todo_id".into()
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    todo.status = TodoStatus::Completed;
                    todo.completed_at = Some(Utc::now());
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("✅ Completed: {}\n\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Todo not found".into()
                    })
                }
            }
            
            "fail" => {
                let todo_id = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id required for fail".into()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Invalid todo_id".into()
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    todo.status = TodoStatus::Failed;
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("❌ Failed: {}\n\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Todo not found".into()
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
            
            "update" => {
                let todo_id = input.todo_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "todo_id required for update".into()
                    })?;
                    
                let uuid = Uuid::parse_str(&todo_id)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Invalid todo_id".into()
                    })?;
                
                let mut todos = self.todos.lock().unwrap();
                
                if let Some(todo) = todos.get_mut(&uuid) {
                    if let Some(status_str) = input.status {
                        todo.status = match status_str.as_str() {
                            "pending" => TodoStatus::Pending,
                            "in_progress" => TodoStatus::InProgress,
                            "completed" => TodoStatus::Completed,
                            "failed" => TodoStatus::Failed,
                            _ => todo.status.clone(),
                        };
                    }
                    
                    if let Some(priority_str) = input.priority {
                        todo.priority = match priority_str.as_str() {
                            "high" => TodoPriority::High,
                            "medium" => TodoPriority::Medium,
                            "low" => TodoPriority::Low,
                            _ => todo.priority.clone(),
                        };
                    }
                    
                    let desc = todo.description.clone();
                    drop(todos);
                    
                    let output = format!("📝 Updated: {}\n\n{}", desc, self.format_todo_list());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Todo not found".into()
                    })
                }
            }
            
            _ => Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Unknown action: {}", input.action)
            })
        }
    }
}
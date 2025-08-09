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
pub struct PlannedTask {
    pub id: Uuid,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub dependencies: Vec<Uuid>,
    pub estimated_effort: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskPriority {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "blocked")]
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanInput {
    action: String,
    #[serde(default)]
    task_description: Option<String>,
    #[serde(default)]
    tasks: Option<Vec<TaskDefinition>>,
    #[serde(default)]
    task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskDefinition {
    description: String,
    priority: TaskPriority,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    estimated_effort: Option<String>,
}

pub struct TodoPlannerTool {
    tasks: Arc<Mutex<HashMap<Uuid, PlannedTask>>>,
    execution_order: Arc<Mutex<Vec<Uuid>>>,
}

impl TodoPlannerTool {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            execution_order: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    fn format_plan(&self) -> String {
        let tasks = self.tasks.lock().unwrap();
        let order = self.execution_order.lock().unwrap();
        
        if tasks.is_empty() {
            return "No tasks planned. Use action: 'plan' to create a comprehensive task plan.".to_string();
        }
        
        let mut output = String::new();
        output.push_str("\n📋 Task Execution Plan\n");
        output.push_str("════════════════════════════════════════════════════════════\n");
        
        // Summary
        let total = tasks.len();
        let completed = tasks.values().filter(|t| t.status == TaskStatus::Completed).count();
        let in_progress = tasks.values().filter(|t| t.status == TaskStatus::InProgress).count();
        let blocked = tasks.values().filter(|t| t.status == TaskStatus::Blocked).count();
        
        output.push_str(&format!("📊 Progress: [{}/{}] completed | {} in progress | {} blocked\n", 
            completed, total, in_progress, blocked));
        
        // Progress bar
        let progress_percent = if total > 0 { (completed * 100) / total } else { 0 };
        let bar_width = 40;
        let filled = (progress_percent * bar_width) / 100;
        let empty = bar_width - filled;
        output.push_str(&format!("   [{}{}] {}%\n\n", 
            "█".repeat(filled), 
            "░".repeat(empty), 
            progress_percent));
        
        // Task list by priority and status
        output.push_str("📝 Task List:\n");
        output.push_str("────────────────────────────────────────────────────────────\n");
        
        // Group tasks by status and priority
        let mut high_priority = Vec::new();
        let mut medium_priority = Vec::new();
        let mut low_priority = Vec::new();
        
        for &task_id in order.iter() {
            if let Some(task) = tasks.get(&task_id) {
                match task.priority {
                    TaskPriority::High => high_priority.push(task),
                    TaskPriority::Medium => medium_priority.push(task),
                    TaskPriority::Low => low_priority.push(task),
                }
            }
        }
        
        // Display tasks by priority
        if !high_priority.is_empty() {
            output.push_str("\n🔴 High Priority:\n");
            for task in high_priority {
                output.push_str(&self.format_task_line(task));
            }
        }
        
        if !medium_priority.is_empty() {
            output.push_str("\n🟡 Medium Priority:\n");
            for task in medium_priority {
                output.push_str(&self.format_task_line(task));
            }
        }
        
        if !low_priority.is_empty() {
            output.push_str("\n🟢 Low Priority:\n");
            for task in low_priority {
                output.push_str(&self.format_task_line(task));
            }
        }
        
        // Show next actionable tasks
        output.push_str("\n\n🎯 Next Actionable Tasks:\n");
        output.push_str("────────────────────────────────────────────────────────────\n");
        
        let actionable: Vec<&PlannedTask> = tasks.values()
            .filter(|t| {
                t.status == TaskStatus::Pending && 
                self.dependencies_met(&tasks, &t.dependencies)
            })
            .take(3)
            .collect();
        
        if actionable.is_empty() {
            output.push_str("   ⚠️ No actionable tasks available\n");
        } else {
            for task in actionable {
                output.push_str(&format!("   → {} (ID: {})\n", task.description, task.id));
                if let Some(effort) = &task.estimated_effort {
                    output.push_str(&format!("     Estimated: {}\n", effort));
                }
            }
        }
        
        output
    }
    
    fn format_task_line(&self, task: &PlannedTask) -> String {
        let status_icon = match task.status {
            TaskStatus::Completed => "✅",
            TaskStatus::InProgress => "🔄",
            TaskStatus::Blocked => "🚫",
            TaskStatus::Pending => "⏺",
        };
        
        // Include shortened task ID for reference (first 8 chars)
        let short_id = task.id.to_string()[..8].to_string();
        let mut line = format!("   {} [{}] {}", status_icon, short_id, task.description);
        
        if let Some(effort) = &task.estimated_effort {
            line.push_str(&format!(" [{}]", effort));
        }
        
        if !task.dependencies.is_empty() {
            line.push_str(" (has dependencies)");
        }
        
        line.push('\n');
        line
    }
    
    fn dependencies_met(&self, tasks: &HashMap<Uuid, PlannedTask>, deps: &[Uuid]) -> bool {
        deps.iter().all(|dep_id| {
            tasks.get(dep_id)
                .map(|t| t.status == TaskStatus::Completed)
                .unwrap_or(false)
        })
    }
    
    fn update_blocked_status(&self) {
        let mut tasks = self.tasks.lock().unwrap();
        let task_ids: Vec<Uuid> = tasks.keys().cloned().collect();
        
        // Collect status updates to apply after checking dependencies
        let mut status_updates = Vec::new();
        
        for task_id in task_ids {
            if let Some(task) = tasks.get(&task_id) {
                if task.status == TaskStatus::Blocked || task.status == TaskStatus::Pending {
                    let dependencies = task.dependencies.clone();
                    let current_status = task.status.clone();
                    let has_deps = !dependencies.is_empty();
                    
                    let deps_met = dependencies.iter().all(|dep_id| {
                        tasks.get(dep_id)
                            .map(|t| t.status == TaskStatus::Completed)
                            .unwrap_or(false)
                    });
                    
                    if deps_met && current_status == TaskStatus::Blocked {
                        status_updates.push((task_id, TaskStatus::Pending));
                    } else if !deps_met && current_status == TaskStatus::Pending && has_deps {
                        status_updates.push((task_id, TaskStatus::Blocked));
                    }
                }
            }
        }
        
        // Apply status updates
        for (task_id, new_status) in status_updates {
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = new_status;
            }
        }
    }
}

impl Default for TodoPlannerTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TodoPlannerTool {
    fn name(&self) -> &str {
        "todo_planner"
    }

    fn description(&self) -> &str {
        "Create and manage comprehensive task execution plans with priorities, dependencies, and progress tracking. Actions: plan, start, complete, status, clear"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["plan", "start", "complete", "status", "clear"],
                    "description": "The action to perform on the task plan"
                },
                "task_description": {
                    "type": "string",
                    "description": "High-level description of what needs to be accomplished (for 'plan' action)"
                },
                "tasks": {
                    "type": "array",
                    "description": "List of tasks to plan (for 'plan' action)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {
                                "type": "string",
                                "description": "Task description"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["high", "medium", "low"],
                                "description": "Task priority"
                            },
                            "dependencies": {
                                "type": "array",
                                "items": {"type": "string"},
                                "description": "List of task descriptions this task depends on"
                            },
                            "estimated_effort": {
                                "type": "string",
                                "description": "Estimated effort (e.g., '5 mins', '1 hour')"
                            }
                        },
                        "required": ["description", "priority"]
                    }
                },
                "task_id": {
                    "type": "string",
                    "description": "The ID of the task to update (for 'start' and 'complete' actions)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> Result<Value> {
        let input: PlanInput = serde_json::from_value(params)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid parameters: {}", e),
            })?;
        
        match input.action.as_str() {
            "plan" => {
                let tasks = input.tasks
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Tasks list is required for plan action".to_string()
                    })?;
                
                if tasks.is_empty() {
                    return Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "Cannot create a plan with no tasks".to_string()
                    });
                }
                
                let desc = input.task_description.unwrap_or_else(|| "Task Execution Plan".to_string());
                
                // Clear existing tasks
                self.tasks.lock().unwrap().clear();
                self.execution_order.lock().unwrap().clear();
                
                // Create task map for dependency resolution
                let mut task_map: HashMap<String, Uuid> = HashMap::new();
                let mut created_tasks = Vec::new();
                
                // First pass: create all tasks
                for task_def in &tasks {
                    let task = PlannedTask {
                        id: Uuid::new_v4(),
                        description: task_def.description.clone(),
                        priority: task_def.priority.clone(),
                        status: TaskStatus::Pending,
                        dependencies: Vec::new(), // Will be filled in second pass
                        estimated_effort: task_def.estimated_effort.clone(),
                        created_at: Utc::now(),
                        started_at: None,
                        completed_at: None,
                    };
                    
                    task_map.insert(task.description.clone(), task.id);
                    created_tasks.push((task, task_def.dependencies.clone()));
                }
                
                // Second pass: resolve dependencies and insert tasks
                let mut tasks_guard = self.tasks.lock().unwrap();
                let mut order_guard = self.execution_order.lock().unwrap();
                
                for (mut task, dep_names) in created_tasks {
                    // Resolve dependency names to IDs
                    for dep_name in dep_names {
                        if let Some(&dep_id) = task_map.get(&dep_name) {
                            task.dependencies.push(dep_id);
                        }
                    }
                    
                    // Set initial status based on dependencies
                    if !task.dependencies.is_empty() {
                        task.status = TaskStatus::Blocked;
                    }
                    
                    let task_id = task.id;
                    tasks_guard.insert(task_id, task);
                    order_guard.push(task_id);
                }
                
                // Create a task list with IDs for the response
                let task_list: Vec<serde_json::Value> = order_guard.iter()
                    .filter_map(|id| {
                        tasks_guard.get(id).map(|task| {
                            json!({
                                "id": task.id.to_string(),
                                "description": task.description,
                                "priority": task.priority,
                                "status": task.status,
                                "dependencies": task.dependencies.iter().map(|d| d.to_string()).collect::<Vec<_>>(),
                                "estimated_effort": task.estimated_effort
                            })
                        })
                    })
                    .collect();
                
                drop(tasks_guard);
                drop(order_guard);
                
                let output = format!("✅ Task plan created: {}\n\n{}", desc, self.format_plan());
                
                Ok(json!({
                    "success": true,
                    "total_tasks": tasks.len(),
                    "tasks": task_list,
                    "message": output
                }))
            }
            
            "start" => {
                let task_id_str = input.task_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "task_id is required for start action".to_string()
                    })?;
                    
                let uuid = Uuid::parse_str(&task_id_str)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Invalid task_id: {}", task_id_str)
                    })?;
                
                let mut tasks = self.tasks.lock().unwrap();
                
                if let Some(task) = tasks.get_mut(&uuid) {
                    if task.status == TaskStatus::Blocked {
                        return Err(BedrockError::ToolError {
                            tool: self.name().to_string(),
                            message: "Cannot start blocked task. Complete dependencies first.".to_string()
                        });
                    }
                    
                    task.status = TaskStatus::InProgress;
                    task.started_at = Some(Utc::now());
                    let desc = task.description.clone();
                    drop(tasks);
                    
                    let output = format!("→ Starting: {}\n\n{}", desc, self.format_plan());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Task not found: {}", task_id_str)
                    })
                }
            }
            
            "complete" => {
                let task_id_str = input.task_id
                    .ok_or_else(|| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: "task_id is required for complete action".to_string()
                    })?;
                    
                let uuid = Uuid::parse_str(&task_id_str)
                    .map_err(|_| BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Invalid task_id: {}", task_id_str)
                    })?;
                
                let mut tasks = self.tasks.lock().unwrap();
                
                if let Some(task) = tasks.get_mut(&uuid) {
                    task.status = TaskStatus::Completed;
                    task.completed_at = Some(Utc::now());
                    let desc = task.description.clone();
                    drop(tasks);
                    
                    // Update blocked status for dependent tasks
                    self.update_blocked_status();
                    
                    let output = format!("✓ Completed: {}\n\n{}", desc, self.format_plan());
                    
                    Ok(json!({
                        "success": true,
                        "message": output
                    }))
                } else {
                    Err(BedrockError::ToolError {
                        tool: self.name().to_string(),
                        message: format!("Task not found: {}", task_id_str)
                    })
                }
            }
            
            "status" => {
                let output = self.format_plan();
                
                Ok(json!({
                    "success": true,
                    "message": output
                }))
            }
            
            "clear" => {
                self.tasks.lock().unwrap().clear();
                self.execution_order.lock().unwrap().clear();
                
                Ok(json!({
                    "success": true,
                    "message": "✅ Task plan cleared"
                }))
            }
            
            _ => Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Unknown action: {}. Valid actions are: plan, start, complete, status, clear", input.action)
            })
        }
    }
}
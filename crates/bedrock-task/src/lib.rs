use bedrock_client::{BedrockClient, ConversationRequest};
use bedrock_config::AgentConfig;
use bedrock_core::{
    BedrockError, CostDetails, Message, MessageRole, Result, Task, TaskResult, TaskStatus,
    TokenStatistics,
};
use bedrock_tools::ToolRegistry;
use chrono::Utc;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Priority {
    High = 3,
    Normal = 2,
    Low = 1,
}

#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub task: Task,
    pub priority: Priority,
    pub queued_at: chrono::DateTime<chrono::Utc>,
}

impl PartialEq for QueuedTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.task_id == other.task.task_id
    }
}

impl Eq for QueuedTask {}

impl PartialOrd for QueuedTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.priority.clone() as u8)
            .cmp(&(other.priority.clone() as u8))
            .then_with(|| other.queued_at.cmp(&self.queued_at))
    }
}

pub struct TaskQueue {
    queue: Arc<Mutex<BinaryHeap<QueuedTask>>>,
    max_size: usize,
}

impl TaskQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            max_size,
        }
    }

    pub async fn enqueue(&self, task: Task, priority: Priority) -> Result<()> {
        let mut queue = self.queue.lock().await;
        
        if queue.len() >= self.max_size {
            return Err(BedrockError::TaskError("Task queue is full".into()));
        }

        let queued_task = QueuedTask {
            task,
            priority,
            queued_at: Utc::now(),
        };

        queue.push(queued_task);
        debug!("Task enqueued, queue size: {}", queue.len());
        Ok(())
    }

    pub async fn dequeue(&self) -> Option<QueuedTask> {
        let mut queue = self.queue.lock().await;
        queue.pop()
    }

    pub async fn len(&self) -> usize {
        let queue = self.queue.lock().await;
        queue.len()
    }

    pub async fn is_empty(&self) -> bool {
        let queue = self.queue.lock().await;
        queue.is_empty()
    }
}

pub struct TaskExecutor {
    bedrock_client: Arc<BedrockClient>,
    #[allow(dead_code)]
    tool_registry: Arc<ToolRegistry>,
    config: Arc<AgentConfig>,
    default_timeout: Duration,
}

impl TaskExecutor {
    pub fn new(
        bedrock_client: Arc<BedrockClient>,
        tool_registry: Arc<ToolRegistry>,
        config: Arc<AgentConfig>,
    ) -> Self {
        Self {
            bedrock_client,
            tool_registry,
            config,
            default_timeout: Duration::from_secs(300),
        }
    }

    #[instrument(skip(self), fields(task_id = %task.task_id))]
    pub async fn execute(&self, task: Task) -> Result<TaskResult> {
        info!("Starting task execution");
        let started_at = Utc::now();
        
        let execution_future = self.execute_internal(task.clone());
        
        match timeout(self.default_timeout, execution_future).await {
            Ok(result) => result,
            Err(_) => {
                error!("Task execution timed out");
                Ok(TaskResult {
                    task_id: task.task_id,
                    status: TaskStatus::Failed,
                    summary: "Task execution timed out".to_string(),
                    conversation: Vec::new(),
                    token_stats: TokenStatistics::default(),
                    cost: CostDetails::default(),
                    started_at,
                    completed_at: Some(Utc::now()),
                    error: Some("Timeout".to_string()),
                })
            }
        }
    }

    async fn execute_internal(&self, task: Task) -> Result<TaskResult> {
        let started_at = Utc::now();
        let mut conversation = Vec::new();
        let mut total_tokens = TokenStatistics::default();

        if !task.context.is_empty() {
            conversation.push(Message {
                role: MessageRole::System,
                content: task.context.clone(),
                timestamp: Utc::now(),
            });
        }

        conversation.push(Message {
            role: MessageRole::User,
            content: task.prompt.clone(),
            timestamp: Utc::now(),
        });

        let request = ConversationRequest {
            model_id: self.config.agent.model.clone(),
            messages: conversation.clone(),
            system_prompt: if task.context.is_empty() {
                None
            } else {
                Some(task.context.clone())
            },
            max_tokens: Some(self.config.agent.max_tokens),
            temperature: Some(self.config.agent.temperature),
        };

        match self.bedrock_client.converse(request).await {
            Ok(response) => {
                conversation.push(Message {
                    role: response.role,
                    content: response.content.clone(),
                    timestamp: Utc::now(),
                });

                total_tokens.input_tokens += response.usage.input_tokens;
                total_tokens.output_tokens += response.usage.output_tokens;
                total_tokens.total_tokens += response.usage.total_tokens;

                let cost = self.calculate_cost(&total_tokens);
                let summary = self.generate_summary(&response.content);

                Ok(TaskResult {
                    task_id: task.task_id,
                    status: TaskStatus::Completed,
                    summary,
                    conversation,
                    token_stats: total_tokens,
                    cost,
                    started_at,
                    completed_at: Some(Utc::now()),
                    error: None,
                })
            }
            Err(e) => {
                error!("Task execution failed: {}", e);
                Ok(TaskResult {
                    task_id: task.task_id,
                    status: TaskStatus::Failed,
                    summary: "Task failed".to_string(),
                    conversation,
                    token_stats: total_tokens,
                    cost: CostDetails::default(),
                    started_at,
                    completed_at: Some(Utc::now()),
                    error: Some(e.to_string()),
                })
            }
        }
    }

    fn calculate_cost(&self, stats: &TokenStatistics) -> CostDetails {
        let model_pricing = self.config.pricing.get(&self.config.agent.model);
        
        match model_pricing {
            Some(pricing) => {
                let input_cost = (stats.input_tokens as f64 / 1000.0) * pricing.input_per_1k;
                let output_cost = (stats.output_tokens as f64 / 1000.0) * pricing.output_per_1k;
                
                CostDetails {
                    input_cost,
                    output_cost,
                    total_cost: input_cost + output_cost,
                    currency: pricing.currency.clone(),
                    model: self.config.agent.model.clone(),
                }
            }
            None => {
                warn!("No pricing found for model: {}", self.config.agent.model);
                CostDetails {
                    model: self.config.agent.model.clone(),
                    ..Default::default()
                }
            }
        }
    }

    fn generate_summary(&self, content: &str) -> String {
        let max_length = 200;
        if content.len() <= max_length {
            content.to_string()
        } else {
            let truncated = &content[..max_length];
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &content[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }
}

pub struct TaskPersistence {
    base_dir: std::path::PathBuf,
}

impl TaskPersistence {
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub async fn save(&self, result: &TaskResult) -> Result<()> {
        let task_dir = self.base_dir.join("tasks");
        tokio::fs::create_dir_all(&task_dir).await
            .map_err(|e| BedrockError::IoError(e))?;

        let file_path = task_dir.join(format!("{}.json", result.task_id));
        let json = serde_json::to_string_pretty(result)
            .map_err(|e| BedrockError::SerializationError(e))?;

        tokio::fs::write(file_path, json).await
            .map_err(|e| BedrockError::IoError(e))?;

        debug!("Task result saved: {}", result.task_id);
        Ok(())
    }

    pub async fn load(&self, task_id: &Uuid) -> Result<TaskResult> {
        let file_path = self.base_dir.join("tasks").join(format!("{}.json", task_id));
        
        let json = tokio::fs::read_to_string(file_path).await
            .map_err(|e| BedrockError::IoError(e))?;
        
        let result = serde_json::from_str(&json)
            .map_err(|e| BedrockError::SerializationError(e))?;
        
        Ok(result)
    }

    pub async fn list_tasks(&self) -> Result<Vec<Uuid>> {
        let task_dir = self.base_dir.join("tasks");
        
        if !task_dir.exists() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::new();
        let mut entries = tokio::fs::read_dir(task_dir).await
            .map_err(|e| BedrockError::IoError(e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| BedrockError::IoError(e))? {
            
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".json") {
                    if let Ok(uuid) = file_name[..file_name.len()-5].parse::<Uuid>() {
                        tasks.push(uuid);
                    }
                }
            }
        }

        Ok(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_queue() {
        let queue = TaskQueue::new(10);
        
        let task1 = Task::new("Task 1");
        let task2 = Task::new("Task 2");
        let task3 = Task::new("Task 3");

        queue.enqueue(task1.clone(), Priority::Low).await.unwrap();
        queue.enqueue(task2.clone(), Priority::High).await.unwrap();
        queue.enqueue(task3.clone(), Priority::Normal).await.unwrap();

        assert_eq!(queue.len().await, 3);

        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.task.prompt, "Task 2");

        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.task.prompt, "Task 3");

        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.task.prompt, "Task 1");

        assert!(queue.is_empty().await);
    }
}
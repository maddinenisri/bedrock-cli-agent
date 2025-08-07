use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: Uuid,
    pub context: String,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            task_id: Uuid::new_v4(),
            context: String::new(),
            prompt: prompt.into(),
            created_at: Utc::now(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = context.into();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub summary: String,
    pub conversation: Vec<Message>,
    pub token_stats: TokenStatistics,
    pub cost: CostDetails,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStatistics {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub cache_hits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDetails {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
    pub model: String,
}

impl Default for CostDetails {
    fn default() -> Self {
        Self {
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            currency: "USD".to_string(),
            model: String::new(),
        }
    }
}

#[derive(Error, Debug)]
pub enum BedrockError {
    #[error("AWS authentication failed: {0}")]
    AuthError(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),
    
    #[error("Tool execution failed for '{tool}': {message}")]
    ToolError { tool: String, message: String },
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Task execution failed: {0}")]
    TaskError(String),
    
    #[error("MCP communication error: {0}")]
    McpError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, BedrockError>;

#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    async fn execute_task(&self, task: Task) -> Result<TaskResult>;
    async fn cancel_task(&self, task_id: &Uuid) -> Result<()>;
    async fn get_task_status(&self, task_id: &Uuid) -> Result<TaskStatus>;
}
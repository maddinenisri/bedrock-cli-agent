use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Token usage statistics for a conversation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageStats {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub total_cost: Option<f64>,
}

/// Represents a conversation's metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    pub id: Uuid,
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub working_directory: String,
    pub message_count: usize,
    pub has_tasks: bool,
    pub task_count: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    #[serde(default)]
    pub token_usage: TokenUsageStats,
}

impl ConversationMetadata {
    pub fn new(model_id: String, system_prompt: Option<String>) -> Self {
        let now = Utc::now();
        let working_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        
        Self {
            id: Uuid::new_v4(),
            model_id,
            system_prompt,
            created_at: now,
            updated_at: now,
            working_directory: working_dir,
            message_count: 0,
            has_tasks: false,
            task_count: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            token_usage: TokenUsageStats::default(),
        }
    }
}

/// A single message entry in the conversation log (JSONL format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub timestamp: DateTime<Utc>,
    pub role: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsageStats>,
}

impl MessageEntry {
    pub fn user(content: String) -> Self {
        Self {
            timestamp: Utc::now(),
            role: "user".to_string(),
            content: serde_json::Value::String(content),
            tool_name: None,
            tool_use_id: None,
            tokens: None,
        }
    }
    
    pub fn assistant(content: String) -> Self {
        Self {
            timestamp: Utc::now(),
            role: "assistant".to_string(),
            content: serde_json::Value::String(content),
            tool_name: None,
            tool_use_id: None,
            tokens: None,
        }
    }
    
    pub fn tool(tool_name: String, tool_use_id: String, result: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            role: "tool".to_string(),
            content: result,
            tool_name: Some(tool_name),
            tool_use_id: Some(tool_use_id),
            tokens: None,
        }
    }
}

/// Summary of a conversation for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub summary: Option<String>,
    pub has_tasks: bool,
    pub task_count: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
}

impl From<&ConversationMetadata> for ConversationSummary {
    fn from(meta: &ConversationMetadata) -> Self {
        Self {
            id: meta.id,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            message_count: meta.message_count,
            summary: None,
            has_tasks: meta.has_tasks,
            task_count: meta.task_count,
            completed_tasks: meta.completed_tasks,
            failed_tasks: meta.failed_tasks,
        }
    }
}

/// Index of all conversations in a directory
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationIndex {
    pub conversations: Vec<ConversationSummary>,
    pub workspace_path: String,
    pub last_updated: DateTime<Utc>,
}

impl ConversationIndex {
    pub fn new() -> Self {
        let workspace_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        
        Self {
            conversations: Vec::new(),
            workspace_path,
            last_updated: Utc::now(),
        }
    }
    
    pub fn add_conversation(&mut self, summary: ConversationSummary) {
        self.conversations.push(summary);
        self.last_updated = Utc::now();
    }
    
    pub fn update_conversation(&mut self, meta: &ConversationMetadata) {
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == meta.id) {
            *conv = ConversationSummary::from(meta);
        } else {
            self.add_conversation(ConversationSummary::from(meta));
        }
        self.last_updated = Utc::now();
    }
}
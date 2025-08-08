use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use aws_smithy_types::Document;
use bedrock_core::{BedrockError, Result};
use serde_json::Value;
use tracing::{debug, info};
use uuid::Uuid;

use crate::metadata::{MessageEntry, TokenUsageStats};
use crate::storage::ConversationStorage;

/// Manages conversation state and persistence
pub struct ConversationManager {
    storage: ConversationStorage,
    conversation_id: Option<Uuid>,
}

impl ConversationManager {
    /// Create a new conversation manager
    pub fn new() -> Result<Self> {
        let storage = ConversationStorage::new()?;
        Ok(Self {
            storage,
            conversation_id: None,
        })
    }
    
    /// Convert AWS Document to JSON Value for serialization
    fn document_to_json(doc: &Document) -> Result<Value> {
        match doc {
            Document::Null => Ok(Value::Null),
            Document::Bool(b) => Ok(Value::Bool(*b)),
            Document::Number(n) => {
                match n {
                    aws_smithy_types::Number::PosInt(u) => Ok(Value::Number((*u).into())),
                    aws_smithy_types::Number::NegInt(i) => Ok(Value::Number((*i).into())),
                    aws_smithy_types::Number::Float(f) => {
                        serde_json::Number::from_f64(*f)
                            .map(Value::Number)
                            .ok_or_else(|| BedrockError::Unknown("Invalid float value".into()))
                    }
                }
            }
            Document::String(s) => Ok(Value::String(s.clone())),
            Document::Array(arr) => {
                let values: Result<Vec<Value>> = arr.iter()
                    .map(Self::document_to_json)
                    .collect();
                Ok(Value::Array(values?))
            }
            Document::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::document_to_json(v)?);
                }
                Ok(Value::Object(map))
            }
        }
    }
    
    /// Start a new conversation
    pub fn start_conversation(
        &mut self,
        model_id: String,
        system_prompt: Option<String>,
    ) -> Result<Uuid> {
        let metadata = self.storage.create_conversation(model_id, system_prompt)?;
        self.conversation_id = Some(metadata.id);
        
        info!("Started new conversation: {}", metadata.id);
        Ok(metadata.id)
    }
    
    /// Resume an existing conversation
    pub fn resume_conversation(&mut self, conversation_id: Uuid) -> Result<Vec<MessageEntry>> {
        // Verify the conversation exists
        let _ = self.storage.load_metadata(&conversation_id)?;
        self.conversation_id = Some(conversation_id);
        
        // Load message history
        let messages = self.storage.read_messages(&conversation_id)?;
        
        info!("Resumed conversation {} with {} messages", 
              conversation_id, messages.len());
        Ok(messages)
    }
    
    /// Add a user message to the conversation
    pub fn add_user_message(&self, content: String) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        let entry = MessageEntry::user(content);
        self.storage.append_message(&conversation_id, &entry)?;
        
        // Update metadata
        let mut metadata = self.storage.load_metadata(&conversation_id)?;
        metadata.message_count += 1;
        metadata.updated_at = chrono::Utc::now();
        self.storage.save_metadata(&metadata)?;
        
        Ok(())
    }
    
    /// Add an assistant message to the conversation
    pub fn add_assistant_message(
        &self,
        content: String,
        tokens: Option<TokenUsageStats>,
    ) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        let mut entry = MessageEntry::assistant(content);
        entry.tokens = tokens.clone();
        self.storage.append_message(&conversation_id, &entry)?;
        
        // Update metadata
        let mut metadata = self.storage.load_metadata(&conversation_id)?;
        metadata.message_count += 1;
        metadata.updated_at = chrono::Utc::now();
        
        // Update token usage
        if let Some(tokens) = tokens {
            metadata.token_usage.input_tokens += tokens.input_tokens;
            metadata.token_usage.output_tokens += tokens.output_tokens;
            metadata.token_usage.total_tokens += tokens.total_tokens;
            
            if let Some(cost) = tokens.total_cost {
                metadata.token_usage.total_cost = Some(
                    metadata.token_usage.total_cost.unwrap_or(0.0) + cost
                );
            }
        }
        
        self.storage.save_metadata(&metadata)?;
        Ok(())
    }
    
    /// Add a tool use/result to the conversation
    pub fn add_tool_message(
        &self,
        tool_name: String,
        tool_use_id: String,
        result: Value,
    ) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        let entry = MessageEntry::tool(tool_name, tool_use_id, result);
        self.storage.append_message(&conversation_id, &entry)?;
        
        // Update metadata
        let mut metadata = self.storage.load_metadata(&conversation_id)?;
        metadata.message_count += 1;
        metadata.updated_at = chrono::Utc::now();
        self.storage.save_metadata(&metadata)?;
        
        Ok(())
    }
    
    /// Save a Bedrock Message to the conversation with optional token usage
    pub fn save_bedrock_message(&self, message: &Message, tokens: Option<TokenUsageStats>) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        let role = match message.role() {
            ConversationRole::User => "user",
            ConversationRole::Assistant => "assistant",
            _ => "system",
        }.to_string();
        
        // Convert content blocks to JSON, properly handling all types
        let content = if !message.content().is_empty() {
            let content_json: Vec<Value> = message.content().iter().map(|block| {
                match block {
                    ContentBlock::Text(text) => {
                        serde_json::json!({
                            "type": "text",
                            "text": text
                        })
                    },
                    ContentBlock::ToolUse(tool_use) => {
                        // Convert the Document input to proper JSON
                        let input_json = Self::document_to_json(tool_use.input())
                            .unwrap_or_else(|_| serde_json::json!({}));
                        
                        serde_json::json!({
                            "type": "tool_use",
                            "tool_use_id": tool_use.tool_use_id(),
                            "name": tool_use.name(),
                            "input": input_json
                        })
                    },
                    ContentBlock::ToolResult(tool_result) => {
                        // Get the actual content from tool result
                        let result_content = tool_result.content()
                            .iter()
                            .filter_map(|c| {
                                if let Ok(text) = c.as_text() {
                                    Some(text.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_result.tool_use_id(),
                            "status": format!("{:?}", tool_result.status()),
                            "content": result_content
                        })
                    },
                    _ => serde_json::json!({
                        "type": "unknown"
                    })
                }
            }).collect();
            serde_json::Value::Array(content_json)
        } else {
            serde_json::Value::Null
        };
        
        let mut entry = MessageEntry {
            timestamp: chrono::Utc::now(),
            role,
            content,
            tool_name: None,
            tool_use_id: None,
            tokens: tokens.clone(),
        };
        
        // For tool messages, extract the tool name and ID
        if message.role() == &ConversationRole::Assistant {
            // Check if this message contains tool use
            for block in message.content() {
                if let Ok(tool_use) = block.as_tool_use() {
                    entry.tool_name = Some(tool_use.name().to_string());
                    entry.tool_use_id = Some(tool_use.tool_use_id().to_string());
                    break; // Just get the first tool for metadata
                }
            }
        }
        
        self.storage.append_message(&conversation_id, &entry)?;
        
        // Update metadata including token usage
        let mut metadata = self.storage.load_metadata(&conversation_id)?;
        metadata.message_count += 1;
        metadata.updated_at = chrono::Utc::now();
        
        // Update token usage if provided
        if let Some(tokens) = tokens {
            metadata.token_usage.input_tokens += tokens.input_tokens;
            metadata.token_usage.output_tokens += tokens.output_tokens;
            metadata.token_usage.total_tokens += tokens.total_tokens;
            
            if let Some(cost) = tokens.total_cost {
                metadata.token_usage.total_cost = Some(
                    metadata.token_usage.total_cost.unwrap_or(0.0) + cost
                );
            }
        }
        
        self.storage.save_metadata(&metadata)?;
        
        Ok(())
    }
    
    /// Save task results associated with the conversation
    pub fn save_task_results(&self, tasks: Value) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        self.storage.save_task_results(&conversation_id, &tasks)?;
        
        // Update metadata
        let mut metadata = self.storage.load_metadata(&conversation_id)?;
        metadata.has_tasks = true;
        if let Some(task_array) = tasks.as_array() {
            metadata.task_count = task_array.len();
            metadata.completed_tasks = task_array.iter()
                .filter(|t| t.get("status") == Some(&Value::String("completed".to_string())))
                .count();
            metadata.failed_tasks = task_array.iter()
                .filter(|t| t.get("status") == Some(&Value::String("failed".to_string())))
                .count();
        }
        metadata.updated_at = chrono::Utc::now();
        self.storage.save_metadata(&metadata)?;
        
        debug!("Saved {} task results for conversation {}", 
               metadata.task_count, conversation_id);
        Ok(())
    }
    
    /// List all conversations for the current workspace
    pub fn list_conversations(&self) -> Result<Vec<crate::metadata::ConversationSummary>> {
        self.storage.list_conversations()
    }
    
    /// Export the current conversation
    pub fn export_conversation(&self, output_path: &std::path::Path) -> Result<()> {
        let conversation_id = self.conversation_id
            .ok_or_else(|| BedrockError::TaskError("No active conversation".to_string()))?;
        
        self.storage.export_conversation(&conversation_id, output_path)
    }
    
    /// Get the current conversation ID
    pub fn current_conversation_id(&self) -> Option<Uuid> {
        self.conversation_id
    }
    
    /// Get the current message count
    pub fn get_message_count(&self) -> usize {
        if let Some(conversation_id) = self.conversation_id {
            if let Ok(metadata) = self.storage.load_metadata(&conversation_id) {
                return metadata.message_count;
            }
        }
        0
    }
}
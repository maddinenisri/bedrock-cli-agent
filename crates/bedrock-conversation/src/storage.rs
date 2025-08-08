use bedrock_core::{BedrockError, Result};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use uuid::Uuid;
use sha2::{Sha256, Digest};
use chrono::Utc;

use crate::metadata::{
    ConversationIndex, ConversationMetadata, ConversationSummary, MessageEntry,
};

/// File-based conversation storage with proper HOME_DIR handling
pub struct ConversationStorage {
    base_dir: PathBuf,
    workspace_key: String,
}

impl ConversationStorage {
    /// Create a new conversation storage instance
    pub fn new() -> Result<Self> {
        let home_dir = std::env::var("HOME_DIR")
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|p| p.join(".bedrock-agent").to_string_lossy().to_string())
                    .unwrap_or_else(|| "./.bedrock-agent".to_string())
            });
        
        let base_dir = PathBuf::from(home_dir).join("conversations");
        let workspace_key = Self::generate_workspace_key()?;
        
        debug!("ConversationStorage initialized: base_dir={:?}, workspace_key={}", 
               base_dir, workspace_key);
        
        Ok(Self {
            base_dir,
            workspace_key,
        })
    }
    
    /// Generate a normalized workspace key using hash + directory name
    fn generate_workspace_key() -> Result<String> {
        let cwd = std::env::current_dir()
            .map_err(|e| BedrockError::IoError(e))?;
        
        // Create hash of full path
        let mut hasher = Sha256::new();
        hasher.update(cwd.to_string_lossy().as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        
        // Get the last component of the path as a readable name
        let name = cwd.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        
        // Sanitize the name for filesystem safety
        let safe_name = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            })
            .collect::<String>();
        
        Ok(format!("{}-{}", &hash[..8], safe_name))
    }
    
    /// Get the directory for the current workspace
    pub fn get_workspace_dir(&self) -> PathBuf {
        self.base_dir.join(&self.workspace_key)
    }
    
    /// Ensure the workspace directory exists
    fn ensure_workspace_dir(&self) -> Result<PathBuf> {
        let dir = self.get_workspace_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| BedrockError::IoError(e))?;
        Ok(dir)
    }
    
    /// Create a new conversation
    pub fn create_conversation(
        &self,
        model_id: String,
        system_prompt: Option<String>,
    ) -> Result<ConversationMetadata> {
        let metadata = ConversationMetadata::new(model_id, system_prompt);
        self.save_metadata(&metadata)?;
        self.update_index(&metadata)?;
        
        info!("Created new conversation: {}", metadata.id);
        Ok(metadata)
    }
    
    /// Save conversation metadata
    pub fn save_metadata(&self, metadata: &ConversationMetadata) -> Result<()> {
        let dir = self.ensure_workspace_dir()?;
        let meta_path = dir.join(format!("{}.meta.json", metadata.id));
        
        let json = serde_json::to_string_pretty(metadata)?;
        fs::write(&meta_path, json)
            .map_err(|e| BedrockError::IoError(e))?;
        
        debug!("Saved metadata for conversation {}", metadata.id);
        Ok(())
    }
    
    /// Load conversation metadata
    pub fn load_metadata(&self, conversation_id: &Uuid) -> Result<ConversationMetadata> {
        let dir = self.get_workspace_dir();
        let meta_path = dir.join(format!("{}.meta.json", conversation_id));
        
        let json = fs::read_to_string(&meta_path)
            .map_err(|e| BedrockError::IoError(e))?;
        
        let metadata: ConversationMetadata = serde_json::from_str(&json)?;
        Ok(metadata)
    }
    
    /// Append a message to the conversation JSONL file
    pub fn append_message(&self, conversation_id: &Uuid, entry: &MessageEntry) -> Result<()> {
        let dir = self.ensure_workspace_dir()?;
        let jsonl_path = dir.join(format!("{}.jsonl", conversation_id));
        
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jsonl_path)
            .map_err(|e| BedrockError::IoError(e))?;
        
        let json = serde_json::to_string(entry)?;
        writeln!(file, "{}", json)?;
        
        debug!("Appended message to conversation {}", conversation_id);
        Ok(())
    }
    
    /// Read all messages from a conversation
    pub fn read_messages(&self, conversation_id: &Uuid) -> Result<Vec<MessageEntry>> {
        let dir = self.get_workspace_dir();
        let jsonl_path = dir.join(format!("{}.jsonl", conversation_id));
        
        if !jsonl_path.exists() {
            return Ok(Vec::new());
        }
        
        let file = fs::File::open(&jsonl_path)
            .map_err(|e| BedrockError::IoError(e))?;
        
        let reader = BufReader::new(file);
        let mut messages = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                let entry: MessageEntry = serde_json::from_str(&line)
                    .map_err(|e| BedrockError::SerializationError(e))?;
                messages.push(entry);
            }
        }
        
        Ok(messages)
    }
    
    /// Save task results associated with a conversation
    pub fn save_task_results(
        &self,
        conversation_id: &Uuid,
        tasks: &serde_json::Value,
    ) -> Result<()> {
        let dir = self.ensure_workspace_dir()?;
        let tasks_path = dir.join(format!("{}.tasks.json", conversation_id));
        
        let json = serde_json::to_string_pretty(tasks)?;
        fs::write(&tasks_path, json)
            .map_err(|e| BedrockError::IoError(e))?;
        
        debug!("Saved tasks for conversation {}", conversation_id);
        Ok(())
    }
    
    /// Update the workspace conversation index
    fn update_index(&self, metadata: &ConversationMetadata) -> Result<()> {
        let dir = self.ensure_workspace_dir()?;
        let index_path = dir.join("index.json");
        
        let mut index = if index_path.exists() {
            let json = fs::read_to_string(&index_path)?;
            serde_json::from_str(&json)?
        } else {
            ConversationIndex::new()
        };
        
        index.update_conversation(metadata);
        
        let json = serde_json::to_string_pretty(&index)?;
        fs::write(&index_path, json)
            .map_err(|e| BedrockError::IoError(e))?;
        
        Ok(())
    }
    
    /// List all conversations for the current workspace
    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let dir = self.get_workspace_dir();
        let index_path = dir.join("index.json");
        
        if !index_path.exists() {
            return Ok(Vec::new());
        }
        
        let json = fs::read_to_string(&index_path)?;
        let index: ConversationIndex = serde_json::from_str(&json)?;
        
        Ok(index.conversations)
    }
    
    /// Delete a conversation and all its associated files
    pub fn delete_conversation(&self, conversation_id: &Uuid) -> Result<()> {
        let dir = self.get_workspace_dir();
        
        // Delete all files associated with this conversation
        let patterns = vec![
            format!("{}.jsonl", conversation_id),
            format!("{}.meta.json", conversation_id),
            format!("{}.tasks.json", conversation_id),
        ];
        
        for pattern in patterns {
            let path = dir.join(pattern);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| BedrockError::IoError(e))?;
            }
        }
        
        // Update index
        let index_path = dir.join("index.json");
        if index_path.exists() {
            let json = fs::read_to_string(&index_path)?;
            let mut index: ConversationIndex = serde_json::from_str(&json)?;
            
            index.conversations.retain(|c| c.id != *conversation_id);
            index.last_updated = Utc::now();
            
            let json = serde_json::to_string_pretty(&index)?;
            fs::write(&index_path, json)?;
        }
        
        info!("Deleted conversation {}", conversation_id);
        Ok(())
    }
    
    /// Export a conversation to a standalone file
    pub fn export_conversation(&self, conversation_id: &Uuid, output_path: &Path) -> Result<()> {
        let metadata = self.load_metadata(conversation_id)?;
        let messages = self.read_messages(conversation_id)?;
        
        let export = serde_json::json!({
            "metadata": metadata,
            "messages": messages,
            "exported_at": Utc::now(),
        });
        
        let json = serde_json::to_string_pretty(&export)?;
        fs::write(output_path, json)
            .map_err(|e| BedrockError::IoError(e))?;
        
        info!("Exported conversation {} to {:?}", conversation_id, output_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_workspace_key_generation() {
        let key = ConversationStorage::generate_workspace_key().unwrap();
        assert!(key.len() > 8);
        assert!(key.contains('-'));
    }
    
    #[test]
    fn test_conversation_creation() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("HOME_DIR", temp_dir.path().to_str().unwrap());
        
        let storage = ConversationStorage::new().unwrap();
        let meta = storage.create_conversation(
            "test-model".to_string(),
            Some("test prompt".to_string()),
        ).unwrap();
        
        assert_eq!(meta.model_id, "test-model");
        assert_eq!(meta.system_prompt, Some("test prompt".to_string()));
        
        // Verify files were created
        let workspace_dir = storage.get_workspace_dir();
        assert!(workspace_dir.join(format!("{}.meta.json", meta.id)).exists());
        assert!(workspace_dir.join("index.json").exists());
    }
}
use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::Tool;

#[derive(Debug, Clone)]
pub struct FileReadTool {
    workspace_dir: PathBuf,
    max_file_size: usize,
}

impl FileReadTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        // Try to canonicalize, but if it fails (file doesn't exist yet), 
        // just use the absolute path
        let canonical = absolute_path.canonicalize()
            .unwrap_or_else(|_| absolute_path.clone());

        // Ensure workspace_dir is also canonical for comparison
        let workspace_canonical = self.workspace_dir.canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());

        if !canonical.starts_with(&workspace_canonical) {
            return Err(BedrockError::ToolError {
                tool: "fs_read".to_string(),
                message: format!("Path outside workspace: {canonical:?}"),
            });
        }

        Ok(canonical)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FileReadArgs {
    path: String,
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "fs_read"
    }

    fn description(&self) -> &str {
        "Read contents of a file from the workspace directory"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read (relative to workspace)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let args: FileReadArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        let path = self.validate_path(Path::new(&args.path))?;
        
        let metadata = tokio::fs::metadata(&path).await
            .map_err(BedrockError::IoError)?;
        
        if metadata.len() > self.max_file_size as u64 {
            return Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("File too large: {} bytes", metadata.len()),
            });
        }

        let content = tokio::fs::read_to_string(&path).await
            .map_err(BedrockError::IoError)?;

        debug!("Read {} bytes from {:?}", content.len(), path);
        
        Ok(json!({
            "content": content,
            "path": path.to_string_lossy(),
            "size": content.len()
        }))
    }
}

#[derive(Debug, Clone)]
pub struct FileWriteTool {
    workspace_dir: PathBuf,
    max_file_size: usize,
}

impl FileWriteTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        // Ensure workspace_dir is canonical for comparison
        let workspace_canonical = self.workspace_dir.canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());

        // For write, we need to check the parent directory
        if let Some(parent) = absolute_path.parent() {
            let parent_canonical = parent.canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            
            if !parent_canonical.starts_with(&workspace_canonical) && !absolute_path.starts_with(&workspace_canonical) {
                return Err(BedrockError::ToolError {
                    tool: "fs_write".to_string(),
                    message: format!("Path outside workspace: {absolute_path:?}"),
                });
            }
        }

        Ok(absolute_path)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FileWriteArgs {
    path: String,
    content: String,
    #[serde(default)]
    append: bool,
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "fs_write"
    }

    fn description(&self) -> &str {
        "Write content to a file in the workspace directory"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write (relative to workspace)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Whether to append to existing file (default: false)",
                    "default": false
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let args: FileWriteArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        if args.content.len() > self.max_file_size {
            return Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Content too large: {} bytes", args.content.len()),
            });
        }

        let path = self.validate_path(Path::new(&args.path))?;
        
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(BedrockError::IoError)?;
        }

        if args.append {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&path)
                .await
                .map_err(BedrockError::IoError)?;
            
            file.write_all(args.content.as_bytes()).await
                .map_err(BedrockError::IoError)?;
        } else {
            tokio::fs::write(&path, &args.content).await
                .map_err(BedrockError::IoError)?;
        }

        debug!("Wrote {} bytes to {:?}", args.content.len(), path);
        
        Ok(json!({
            "success": true,
            "path": path.to_string_lossy(),
            "bytes_written": args.content.len(),
            "append": args.append
        }))
    }
}

#[derive(Debug, Clone)]
pub struct FileListTool {
    workspace_dir: PathBuf,
}

impl FileListTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        let canonical = absolute_path.canonicalize()
            .unwrap_or(absolute_path.clone());

        // Ensure workspace_dir is also canonical for comparison
        let workspace_canonical = self.workspace_dir.canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());

        if !canonical.starts_with(&workspace_canonical) {
            return Err(BedrockError::ToolError {
                tool: "fs_list".to_string(),
                message: format!("Path outside workspace: {canonical:?}"),
            });
        }

        Ok(canonical)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FileListArgs {
    #[serde(default = "default_path")]
    path: String,
}

fn default_path() -> String {
    ".".to_string()
}

#[async_trait]
impl Tool for FileListTool {
    fn name(&self) -> &str {
        "fs_list"
    }

    fn description(&self) -> &str {
        "List files and directories in the workspace"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to list (relative to workspace, default: '.')",
                    "default": "."
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let args: FileListArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        let path = self.validate_path(Path::new(&args.path))?;
        
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&path).await
            .map_err(BedrockError::IoError)?;

        while let Some(entry) = dir.next_entry().await
            .map_err(BedrockError::IoError)? {
            
            let metadata = entry.metadata().await
                .map_err(BedrockError::IoError)?;
            
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "type": if metadata.is_dir() { "directory" } else { "file" },
                "size": metadata.len()
            }));
        }

        Ok(json!({
            "path": path.to_string_lossy(),
            "entries": entries
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_read_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "Hello, World!").await.unwrap();

        let tool = FileReadTool::new(temp_dir.path());
        let args = json!({ "path": "test.txt" });
        
        let result = tool.execute(args).await.unwrap();
        assert_eq!(result["content"], "Hello, World!");
    }

    #[tokio::test]
    async fn test_file_write_tool() {
        let temp_dir = TempDir::new().unwrap();
        
        let tool = FileWriteTool::new(temp_dir.path());
        let args = json!({
            "path": "output.txt",
            "content": "Test content"
        });
        
        let result = tool.execute(args).await.unwrap();
        assert_eq!(result["success"], true);
        
        let content = tokio::fs::read_to_string(temp_dir.path().join("output.txt"))
            .await.unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_file_list_tool() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("file1.txt"), "content1").await.unwrap();
        tokio::fs::write(temp_dir.path().join("file2.txt"), "content2").await.unwrap();
        tokio::fs::create_dir(temp_dir.path().join("subdir")).await.unwrap();

        let tool = FileListTool::new(temp_dir.path());
        let args = json!({ "path": "." });
        
        let result = tool.execute(args).await.unwrap();
        let entries = result["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
    }
}
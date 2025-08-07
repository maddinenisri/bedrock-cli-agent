use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::Tool;

#[derive(Debug, Clone)]
pub struct GrepTool {
    workspace_dir: PathBuf,
    max_results: usize,
}

impl GrepTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            max_results: 1000,
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        if !absolute_path.starts_with(&self.workspace_dir) {
            return Err(BedrockError::ToolError {
                tool: "grep".to_string(),
                message: format!("Path outside workspace: {absolute_path:?}"),
            });
        }

        Ok(absolute_path)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    include_line_numbers: bool,
}

fn default_path() -> String {
    ".".to_string()
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for patterns in files using grep"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for (regex supported)"
                },
                "path": {
                    "type": "string",
                    "description": "Path to search in (default: current directory)",
                    "default": "."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search",
                    "default": false
                },
                "include_line_numbers": {
                    "type": "boolean",
                    "description": "Include line numbers in results",
                    "default": false
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let args: GrepArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        let search_path = self.validate_path(Path::new(&args.path))?;

        let mut cmd = Command::new("grep");
        cmd.arg("-r")
            .arg("--max-count").arg(self.max_results.to_string());

        if args.case_insensitive {
            cmd.arg("-i");
        }
        if args.include_line_numbers {
            cmd.arg("-n");
        }

        cmd.arg(&args.pattern)
            .arg(&search_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Failed to execute grep: {e}"),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stderr.is_empty() && !output.status.success() {
            warn!("Grep error: {}", stderr);
        }

        let lines: Vec<&str> = stdout.lines()
            .take(self.max_results)
            .collect();

        debug!("Grep found {} matches", lines.len());

        Ok(json!({
            "matches": lines,
            "count": lines.len(),
            "pattern": args.pattern,
            "path": search_path.to_string_lossy()
        }))
    }
}

#[derive(Debug, Clone)]
pub struct FindTool {
    workspace_dir: PathBuf,
    max_results: usize,
}

impl FindTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            max_results: 1000,
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        if !absolute_path.starts_with(&self.workspace_dir) {
            return Err(BedrockError::ToolError {
                tool: "find".to_string(),
                message: format!("Path outside workspace: {absolute_path:?}"),
            });
        }

        Ok(absolute_path)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FindArgs {
    #[serde(default = "default_pattern")]
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(rename = "type")]
    #[serde(default)]
    file_type: Option<String>,
    #[serde(default)]
    max_depth: Option<usize>,
}

fn default_pattern() -> String {
    "*".to_string()
}

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "Find files and directories by name pattern"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Name pattern to search for (wildcards supported)",
                    "default": "*"
                },
                "path": {
                    "type": "string",
                    "description": "Path to search in (default: current directory)",
                    "default": "."
                },
                "type": {
                    "type": "string",
                    "description": "Type: 'f' for files, 'd' for directories",
                    "enum": ["f", "d"]
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum depth to search"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let args: FindArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        let search_path = self.validate_path(Path::new(&args.path))?;

        let mut cmd = Command::new("find");
        cmd.arg(&search_path);

        if let Some(max_depth) = args.max_depth {
            cmd.arg("-maxdepth").arg(max_depth.to_string());
        }

        if let Some(file_type) = &args.file_type {
            cmd.arg("-type").arg(file_type);
        }

        cmd.arg("-name").arg(&args.pattern);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Failed to execute find: {e}"),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stderr.is_empty() && !output.status.success() {
            warn!("Find error: {}", stderr);
        }

        let mut paths: Vec<String> = stdout.lines()
            .take(self.max_results)
            .map(|line| {
                // Strip workspace prefix for cleaner output
                if let Ok(relative) = Path::new(line).strip_prefix(&self.workspace_dir) {
                    relative.to_string_lossy().to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();

        paths.sort();

        debug!("Find found {} files", paths.len());

        Ok(json!({
            "files": paths,
            "count": paths.len(),
            "pattern": args.pattern,
            "path": search_path.to_string_lossy()
        }))
    }
}

#[derive(Debug, Clone)]
pub struct RipgrepTool {
    workspace_dir: PathBuf,
    max_results: usize,
}

impl RipgrepTool {
    pub fn new(workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            workspace_dir: workspace_dir.into(),
            max_results: 1000,
        }
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_dir.join(path)
        };

        if !absolute_path.starts_with(&self.workspace_dir) {
            return Err(BedrockError::ToolError {
                tool: "rg".to_string(),
                message: format!("Path outside workspace: {absolute_path:?}"),
            });
        }

        Ok(absolute_path)
    }

    async fn check_ripgrep_available() -> bool {
        Command::new("rg")
            .arg("--version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RipgrepArgs {
    pattern: String,
    #[serde(default = "default_path")]
    path: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    file_type: Option<String>,
    #[serde(default)]
    context_lines: Option<usize>,
}

#[async_trait]
impl Tool for RipgrepTool {
    fn name(&self) -> &str {
        "rg"
    }

    fn description(&self) -> &str {
        "Fast search using ripgrep (if available)"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for (regex supported)"
                },
                "path": {
                    "type": "string",
                    "description": "Path to search in (default: current directory)",
                    "default": "."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search",
                    "default": false
                },
                "file_type": {
                    "type": "string",
                    "description": "File type to search (e.g., 'rust', 'python', 'js')"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to show"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        if !Self::check_ripgrep_available().await {
            return Err(BedrockError::ToolError {
                tool: self.name().to_string(),
                message: "ripgrep (rg) is not installed".to_string(),
            });
        }

        let args: RipgrepArgs = serde_json::from_value(args)
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Invalid arguments: {e}"),
            })?;

        let search_path = self.validate_path(Path::new(&args.path))?;

        let mut cmd = Command::new("rg");
        cmd.arg("--max-count").arg(self.max_results.to_string())
            .arg("--no-heading")
            .arg("--line-number");

        if args.case_insensitive {
            cmd.arg("-i");
        }

        if let Some(file_type) = &args.file_type {
            cmd.arg("-t").arg(file_type);
        }

        if let Some(context) = args.context_lines {
            cmd.arg("-C").arg(context.to_string());
        }

        cmd.arg(&args.pattern)
            .arg(&search_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await
            .map_err(|e| BedrockError::ToolError {
                tool: self.name().to_string(),
                message: format!("Failed to execute ripgrep: {e}"),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stderr.is_empty() && !output.status.success() {
            warn!("Ripgrep error: {}", stderr);
        }

        let lines: Vec<&str> = stdout.lines()
            .take(self.max_results)
            .collect();

        debug!("Ripgrep found {} matches", lines.len());

        Ok(json!({
            "matches": lines,
            "count": lines.len(),
            "pattern": args.pattern,
            "path": search_path.to_string_lossy()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_find_tool() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("test1.txt"), "content").await.unwrap();
        tokio::fs::write(temp_dir.path().join("test2.txt"), "content").await.unwrap();
        tokio::fs::write(temp_dir.path().join("other.md"), "content").await.unwrap();

        let tool = FindTool::new(temp_dir.path());
        let args = json!({
            "pattern": "*.txt",
            "path": "."
        });

        let result = tool.execute(args).await.unwrap();
        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
    }
}
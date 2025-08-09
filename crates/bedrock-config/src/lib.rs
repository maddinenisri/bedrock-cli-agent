mod env_substitution;

use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use env_substitution::substitute_env_vars;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent: AgentSettings,
    pub aws: AwsSettings,
    pub tools: ToolSettings,
    pub pricing: HashMap<String, ModelPricing>,
    #[serde(default)]
    pub limits: LimitSettings,
    #[serde(default)]
    pub paths: PathSettings,
    #[serde(default)]
    pub mcp: McpSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub name: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

impl AgentSettings {
    pub fn get_system_prompt(&self) -> String {
        // Base system prompt with todo_planner instructions
        let base_prompt = format!(
            "You are {}, an AI assistant with advanced task planning and execution capabilities.\n\n\
            ## CRITICAL: Task Planning Protocol\n\n\
            When given ANY task that involves multiple steps, you MUST use the todo_planner tool:\n\n\
            ### todo_planner Usage:\n\
            1. **plan** - Create comprehensive task plan with dependencies\n\
               - Returns task IDs (UUIDs) and visual progress display\n\
            2. **start** - Mark task as in progress (use task_id from plan)\n\
            3. **complete** - Mark task as completed (automatically unblocks dependencies)\n\
            4. **status** - Show current progress\n\n\
            ### Workflow:\n\
            1. Analyze request and create plan with todo_planner\n\
            2. Save the returned task IDs\n\
            3. For each task: start → execute → complete\n\
            4. Check status for newly unblocked tasks\n\n\
            ### Important:\n\
            - Task IDs are UUIDs returned by the planner, not descriptions\n\
            - Dependencies automatically manage blocking/unblocking\n\
            - High priority tasks should be done first when possible\n\n\
            You can execute commands, read/write files, and search codebases.",
            self.name
        );

        // Append custom prompt if provided
        if let Some(ref custom_prompt) = self.system_prompt {
            format!("{}\n\n## Additional Instructions:\n{}", base_prompt, custom_prompt)
        } else {
            base_prompt
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsSettings {
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_arn: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettings {
    pub allowed: Vec<String>,
    #[serde(default)]
    pub permissions: HashMap<String, ToolPermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    pub permission: Permission,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Allow,
    Ask,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_1k: f64,
    pub output_per_1k: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitSettings {
    #[serde(default = "default_max_tpm")]
    pub max_tpm: usize,
    #[serde(default = "default_max_rpm")]
    pub max_rpm: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_limit: Option<f64>,
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathSettings {
    #[serde(default = "default_home_dir")]
    pub home_dir: PathBuf,
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub config_files: Vec<String>,
    #[serde(default)]
    pub servers: Vec<String>,
    #[serde(default)]
    pub inline_servers: HashMap<String, serde_json::Value>,
    #[serde(default = "default_max_tools")]
    pub max_tools: usize,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            config_files: vec![],
            servers: vec![],
            inline_servers: HashMap::new(),
            max_tools: default_max_tools(),
        }
    }
}

impl AgentConfig {
    pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| BedrockError::ConfigError(format!("Failed to read config file: {e}")))?;
        
        // Parse YAML to serde_json::Value for env var substitution
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to parse YAML: {e}")))?;
        
        // Convert to JSON value for processing
        let mut json_value = serde_json::to_value(yaml_value)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to convert YAML to JSON: {e}")))?;
        
        // Apply environment variable substitution
        substitute_env_vars(&mut json_value)?;
        
        // Convert back to config struct
        let config: Self = serde_json::from_value(json_value)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to deserialize config: {e}")))?;
        
        config.validate()?;
        
        Ok(config)
    }

    pub fn from_yaml_str(yaml: &str) -> Result<Self> {
        // Parse YAML to serde_json::Value for env var substitution
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to parse YAML: {e}")))?;
        
        // Convert to JSON value for processing
        let mut json_value = serde_json::to_value(yaml_value)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to convert YAML to JSON: {e}")))?;
        
        // Apply environment variable substitution
        substitute_env_vars(&mut json_value)?;
        
        // Convert back to config struct
        let config: Self = serde_json::from_value(json_value)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to deserialize config: {e}")))?;
        
        config.validate()?;
        
        Ok(config)
    }


    fn validate(&self) -> Result<()> {
        if self.agent.name.is_empty() {
            return Err(BedrockError::ConfigError("Agent name cannot be empty".into()));
        }
        if self.agent.model.is_empty() {
            return Err(BedrockError::ConfigError("Model cannot be empty".into()));
        }
        if self.aws.region.is_empty() {
            return Err(BedrockError::ConfigError("AWS region cannot be empty".into()));
        }
        if self.agent.temperature < 0.0 || self.agent.temperature > 1.0 {
            return Err(BedrockError::ConfigError("Temperature must be between 0.0 and 1.0".into()));
        }
        Ok(())
    }

    pub fn default_config_path() -> PathBuf {
        let home_dir = std::env::var("HOME_DIR")
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .map(|p| p.join(".bedrock-agent").to_string_lossy().to_string())
                    .unwrap_or_else(|| "./.bedrock-agent".to_string())
            });
        PathBuf::from(home_dir).join("agent.yaml")
    }
}

impl Default for LimitSettings {
    fn default() -> Self {
        Self {
            max_tpm: default_max_tpm(),
            max_rpm: default_max_rpm(),
            budget_limit: None,
            alert_threshold: default_alert_threshold(),
        }
    }
}

impl Default for PathSettings {
    fn default() -> Self {
        Self {
            home_dir: default_home_dir(),
            workspace_dir: default_workspace_dir(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent: AgentSettings {
                name: "bedrock-agent".to_string(),
                model: "us.anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
                temperature: default_temperature(),
                max_tokens: default_max_tokens(),
                system_prompt: None,
            },
            aws: AwsSettings {
                region: "us-east-1".to_string(),
                profile: None,
                role_arn: None,
            },
            tools: ToolSettings {
                allowed: vec![
                    "fs_read".to_string(),
                    "fs_write".to_string(),
                    "fs_list".to_string(),
                    "grep".to_string(),
                    "find".to_string(),
                ],
                permissions: HashMap::new(),
            },
            pricing: {
                let mut pricing = HashMap::new();
                pricing.insert(
                    "us.anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
                    ModelPricing {
                        input_per_1k: 0.003,
                        output_per_1k: 0.015,
                        currency: default_currency(),
                    },
                );
                pricing
            },
            limits: LimitSettings::default(),
            paths: PathSettings::default(),
            mcp: McpSettings::default(),
        }
    }
}

fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> usize { 4096 }
fn default_currency() -> String { "USD".to_string() }
fn default_max_tpm() -> usize { 100_000 }
fn default_max_rpm() -> usize { 100 }
fn default_alert_threshold() -> f64 { 0.8 }
fn default_max_tools() -> usize { 64 }  // AWS Bedrock limit for most models

fn default_home_dir() -> PathBuf {
    std::env::var("HOME_DIR")
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|p| p.join(".bedrock-agent").to_string_lossy().to_string())
                .unwrap_or_else(|| "./.bedrock-agent".to_string())
        })
        .into()
}

fn default_workspace_dir() -> PathBuf {
    std::env::var("WORKSPACE_DIR")
        .unwrap_or_else(|_| "./workspace".to_string())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
agent:
  name: test-agent
  model: claude-3-sonnet
  temperature: 0.5
  max_tokens: 2048

aws:
  region: us-east-1
  profile: default

tools:
  allowed:
    - fs_read
    - fs_write
  permissions:
    fs_write:
      permission: allow
      constraint: workspace_only

pricing:
  claude-3-sonnet:
    input_per_1k: 0.003
    output_per_1k: 0.015
"#;

        let config = AgentConfig::from_yaml_str(yaml).unwrap();
        assert_eq!(config.agent.name, "test-agent");
        assert_eq!(config.agent.model, "claude-3-sonnet");
        assert_eq!(config.agent.temperature, 0.5);
        assert_eq!(config.tools.allowed.len(), 2);
    }

    #[test]
    fn test_validation() {
        let yaml = r#"
agent:
  name: ""
  model: claude-3-sonnet

aws:
  region: us-east-1

tools:
  allowed: []

pricing: {}
"#;

        let result = AgentConfig::from_yaml_str(yaml);
        assert!(result.is_err());
    }
}
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub name: String,
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
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

impl AgentConfig {
    pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| BedrockError::ConfigError(format!("Failed to read config file: {}", e)))?;
        
        let mut config: AgentConfig = serde_yaml::from_str(&content)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to parse YAML: {}", e)))?;
        
        config.expand_env_vars();
        config.validate()?;
        
        Ok(config)
    }

    pub fn from_str(yaml: &str) -> Result<Self> {
        let mut config: AgentConfig = serde_yaml::from_str(yaml)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to parse YAML: {}", e)))?;
        
        config.expand_env_vars();
        config.validate()?;
        
        Ok(config)
    }

    fn expand_env_vars(&mut self) {
        if let Ok(home_dir) = env::var("HOME_DIR") {
            self.paths.home_dir = PathBuf::from(home_dir);
        }
        if let Ok(workspace_dir) = env::var("WORKSPACE_DIR") {
            self.paths.workspace_dir = PathBuf::from(workspace_dir);
        }
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
        let home_dir = env::var("HOME_DIR")
            .unwrap_or_else(|_| env::var("HOME").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(home_dir).join(".bedrock-agent").join("agent.yaml")
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

fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> usize { 4096 }
fn default_currency() -> String { "USD".to_string() }
fn default_max_tpm() -> usize { 100_000 }
fn default_max_rpm() -> usize { 100 }
fn default_alert_threshold() -> f64 { 0.8 }

fn default_home_dir() -> PathBuf {
    env::var("HOME_DIR")
        .unwrap_or_else(|_| env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .into()
}

fn default_workspace_dir() -> PathBuf {
    env::var("WORKSPACE_DIR")
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

        let config = AgentConfig::from_str(yaml).unwrap();
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

        let result = AgentConfig::from_str(yaml);
        assert!(result.is_err());
    }
}
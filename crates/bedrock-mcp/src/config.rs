//! MCP configuration structures
//! 
//! Supports the standard Amazon Q / Claude Code configuration format
//! with environment variable substitution and multi-level loading.

use bedrock_core::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

use crate::transport::TransportConfig;

/// MCP servers configuration container
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Map of server name to configuration
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

/// Individual MCP server configuration
/// Supports both stdio and SSE transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    /// Stdio-based server (process communication)
    Stdio {
        /// Command to execute (e.g., "npx", "node", "/path/to/binary")
        command: String,
        
        /// Arguments to pass to the command
        #[serde(default)]
        args: Vec<String>,
        
        /// Environment variables for the process
        #[serde(default)]
        env: HashMap<String, String>,
        
        /// Timeout in milliseconds (default: 30000)
        #[serde(default = "default_timeout")]
        timeout: u64,
        
        /// Whether this server is disabled
        #[serde(default)]
        disabled: bool,
        
        /// Optional health check configuration
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health_check: Option<HealthCheckConfig>,
        
        /// Optional restart policy
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restart_policy: Option<RestartPolicy>,
    },
    
    /// SSE-based server (HTTP Server-Sent Events)
    Sse {
        /// Transport type (can be "sse" or omitted)
        #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
        transport_type: Option<String>,
        
        /// URL of the SSE endpoint
        url: String,
        
        /// Headers to include in requests
        #[serde(default)]
        headers: HashMap<String, String>,
        
        /// Timeout in milliseconds (default: 30000)
        #[serde(default = "default_timeout")]
        timeout: u64,
        
        /// Whether this server is disabled
        #[serde(default)]
        disabled: bool,
        
        /// Optional health check configuration
        #[serde(default, skip_serializing_if = "Option::is_none")]
        health_check: Option<HealthCheckConfig>,
        
        /// Optional restart policy
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restart_policy: Option<RestartPolicy>,
    },
}

impl McpServerConfig {
    /// Check if server is disabled
    pub fn is_disabled(&self) -> bool {
        match self {
            McpServerConfig::Stdio { disabled, .. } => *disabled,
            McpServerConfig::Sse { disabled, .. } => *disabled,
        }
    }
    
    /// Get server timeout
    pub fn timeout(&self) -> u64 {
        match self {
            McpServerConfig::Stdio { timeout, .. } => *timeout,
            McpServerConfig::Sse { timeout, .. } => *timeout,
        }
    }
    
    /// Get health check configuration
    pub fn health_check(&self) -> Option<&HealthCheckConfig> {
        match self {
            McpServerConfig::Stdio { health_check, .. } => health_check.as_ref(),
            McpServerConfig::Sse { health_check, .. } => health_check.as_ref(),
        }
    }
    
    /// Get restart policy
    pub fn restart_policy(&self) -> Option<&RestartPolicy> {
        match self {
            McpServerConfig::Stdio { restart_policy, .. } => restart_policy.as_ref(),
            McpServerConfig::Sse { restart_policy, .. } => restart_policy.as_ref(),
        }
    }
    
    /// Convert to transport configuration
    pub fn to_transport_config(&self) -> TransportConfig {
        match self {
            McpServerConfig::Stdio { command, args, env, timeout, .. } => {
                TransportConfig::Stdio {
                    command: command.clone(),
                    args: args.clone(),
                    env: env.clone(),
                    timeout: *timeout,
                }
            }
            McpServerConfig::Sse { url, headers, timeout, .. } => {
                TransportConfig::Sse {
                    url: url.clone(),
                    headers: headers.clone(),
                    timeout: *timeout,
                }
            }
        }
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Health check interval in seconds
    #[serde(default = "default_health_interval")]
    pub interval: u64,
    
    /// Health check timeout in seconds
    #[serde(default = "default_health_timeout")]
    pub timeout: u64,
    
    /// Maximum consecutive failures before restart
    #[serde(default = "default_max_failures")]
    pub max_failures: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            interval: default_health_interval(),
            timeout: default_health_timeout(),
            max_failures: default_max_failures(),
        }
    }
}

/// Restart policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    /// Maximum number of restart attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Initial delay between retries in seconds
    #[serde(default = "default_initial_delay")]
    pub initial_delay: u64,
    
    /// Maximum delay between retries in seconds
    #[serde(default = "default_max_delay")]
    pub max_delay: u64,
    
    /// Backoff strategy
    #[serde(default)]
    pub backoff: BackoffStrategy,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay: default_initial_delay(),
            max_delay: default_max_delay(),
            backoff: BackoffStrategy::Exponential,
        }
    }
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackoffStrategy {
    Linear,
    Exponential,
    Fixed,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential
    }
}

// Default values
fn default_timeout() -> u64 { 30000 }
fn default_health_interval() -> u64 { 60 }
fn default_health_timeout() -> u64 { 5 }
fn default_max_failures() -> u32 { 3 }
fn default_max_retries() -> u32 { 3 }
fn default_initial_delay() -> u64 { 1 }
fn default_max_delay() -> u64 { 30 }

impl McpConfig {
    /// Create empty configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Load configuration from a YAML file
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading MCP configuration from: {}", path.display());
        
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| bedrock_core::BedrockError::ConfigError(
                format!("Failed to read MCP config file {}: {}", path.display(), e)
            ))?;
        
        let config: McpConfig = serde_yaml::from_str(&content)
            .map_err(|e| bedrock_core::BedrockError::ConfigError(
                format!("Failed to parse MCP config YAML from {}: {}", path.display(), e)
            ))?;
        
        Ok(config)
    }
    
    /// Load all YAML files from a directory
    pub async fn load_from_directory<P: AsRef<Path>>(dir: P) -> Result<Vec<Self>> {
        let dir = dir.as_ref();
        let mut configs = Vec::new();
        
        if !dir.exists() {
            return Ok(configs);
        }
        
        let mut entries = tokio::fs::read_dir(dir).await
            .map_err(|e| bedrock_core::BedrockError::ConfigError(
                format!("Failed to read directory {}: {}", dir.display(), e)
            ))?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Only process YAML files
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    match Self::load_from_file(&path).await {
                        Ok(config) => {
                            info!("Loaded MCP config from: {}", path.display());
                            configs.push(config);
                        }
                        Err(e) => {
                            warn!("Failed to load MCP config from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
        
        Ok(configs)
    }
    
    /// Merge another configuration into this one
    /// Other configuration takes precedence (overrides existing)
    pub fn merge(&mut self, other: McpConfig) {
        for (name, config) in other.mcp_servers {
            self.mcp_servers.insert(name, config);
        }
    }
    
    /// Get enabled servers (not disabled)
    pub fn enabled_servers(&self) -> HashMap<String, McpServerConfig> {
        self.mcp_servers
            .iter()
            .filter(|(_, config)| !config.is_disabled())
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_config_parsing() {
        let yaml = r#"
mcpServers:
  filesystem:
    command: npx
    args: ["@modelcontextprotocol/server-filesystem", "--stdio"]
    env:
      WORKSPACE: "/tmp"
    timeout: 30000
    disabled: false
"#;

        let config: McpConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("filesystem"));
        
        let server_config = &config.mcp_servers["filesystem"];
        assert!(!server_config.is_disabled());
        assert_eq!(server_config.timeout(), 30000);
    }

    #[test]
    fn test_sse_config_parsing() {
        let yaml = r#"
mcpServers:
  github:
    type: sse
    url: http://localhost:8080
    headers:
      Authorization: Bearer token123
    timeout: 60000
"#;

        let config: McpConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("github"));
        
        let server_config = &config.mcp_servers["github"];
        assert!(!server_config.is_disabled());
        assert_eq!(server_config.timeout(), 60000);
    }

    #[test]
    fn test_health_check_config() {
        let yaml = r#"
mcpServers:
  test:
    command: echo
    args: ["test"]
    health_check:
      interval: 30
      timeout: 10
      max_failures: 5
"#;

        let config: McpConfig = serde_yaml::from_str(yaml).unwrap();
        let server_config = &config.mcp_servers["test"];
        
        let health_check = server_config.health_check().unwrap();
        assert_eq!(health_check.interval, 30);
        assert_eq!(health_check.timeout, 10);
        assert_eq!(health_check.max_failures, 5);
    }
}
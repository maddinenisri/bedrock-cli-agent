//! MCP Manager for handling multiple MCP servers
//! 
//! Manages multiple MCP servers with configuration loading,
//! lifecycle management, health monitoring, and tool registration.

use bedrock_core::{BedrockError, Result};
use bedrock_tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::client::McpClient;
use crate::config::{BackoffStrategy, McpConfig, McpServerConfig};
use crate::tool_wrapper::McpToolWrapper;

/// Handle to a running MCP server
pub struct McpServerHandle {
    /// Server name
    pub name: String,
    
    /// MCP client
    pub client: Arc<RwLock<McpClient>>,
    
    /// Discovered tool names
    pub tools: Vec<String>,
    
    /// Health monitor task handle (if enabled)
    pub health_monitor: Option<JoinHandle<()>>,
    
    /// Restart count for tracking retries
    pub restart_count: u32,
}

/// MCP Manager for handling multiple MCP servers
pub struct McpManager {
    /// Running servers indexed by name
    servers: Arc<RwLock<HashMap<String, McpServerHandle>>>,
    
    /// Tool registry for registering discovered tools
    tool_registry: Arc<ToolRegistry>,
    
    /// Configuration (merged from all sources)
    config: Arc<RwLock<McpConfig>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            tool_registry,
            config: Arc::new(RwLock::new(McpConfig::new())),
        }
    }
    
    /// Load configuration from a specific file
    pub async fn load_config_file(&mut self, path: &str) -> Result<()> {
        info!("Loading MCP configuration from: {}", path);
        
        let config = McpConfig::load_from_file(path).await?;
        
        // Merge with existing config
        self.config.write().await.merge(config);
        
        Ok(())
    }
    
    /// Load configurations from a directory
    pub async fn load_config_directory(&mut self, dir: &str) -> Result<()> {
        info!("Loading MCP configurations from directory: {}", dir);
        
        let configs = McpConfig::load_from_directory(dir).await?;
        
        let mut config = self.config.write().await;
        for c in configs {
            config.merge(c);
        }
        
        Ok(())
    }
    
    /// Add servers from agent configuration
    pub async fn add_servers_from_config(&mut self, servers: HashMap<String, McpServerConfig>) -> Result<()> {
        info!("Adding {} MCP servers from agent configuration", servers.len());
        
        let mut config = self.config.write().await;
        for (name, server_config) in servers {
            config.mcp_servers.insert(name, server_config);
        }
        
        Ok(())
    }
    
    /// Start specified servers (or all enabled if empty)
    pub async fn start_servers(&mut self, server_names: Vec<String>) -> Result<()> {
        let config = self.config.read().await.clone();
        let enabled_servers = config.enabled_servers();
        
        if enabled_servers.is_empty() {
            info!("No enabled MCP servers to start");
            return Ok(());
        }
        
        // Filter to specified servers if provided
        let servers_to_start: HashMap<String, McpServerConfig> = if server_names.is_empty() {
            enabled_servers
        } else {
            enabled_servers
                .into_iter()
                .filter(|(name, _)| server_names.contains(name))
                .collect()
        };
        
        info!("Starting {} MCP servers", servers_to_start.len());
        
        let mut started = 0;
        let mut failed = 0;
        
        for (name, server_config) in servers_to_start {
            match self.start_server_with_retry(name.clone(), server_config.clone()).await {
                Ok(()) => {
                    started += 1;
                }
                Err(e) => {
                    error!("Failed to start MCP server '{}': {}", name, e);
                    failed += 1;
                }
            }
        }
        
        info!(
            "MCP server startup complete: {} started, {} failed",
            started, failed
        );
        
        if started == 0 && failed > 0 {
            return Err(BedrockError::McpError("Failed to start any MCP servers".into()));
        }
        
        Ok(())
    }
    
    /// Start a specific MCP server with retry logic
    async fn start_server_with_retry(&mut self, name: String, config: McpServerConfig) -> Result<()> {
        let restart_policy = config.restart_policy().cloned().unwrap_or_default();
        let mut retry_count = 0;
        let mut delay = restart_policy.initial_delay;
        
        loop {
            match self.start_server(name.clone(), config.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if retry_count >= restart_policy.max_retries {
                        error!(
                            "Failed to start MCP server '{}' after {} retries: {}",
                            name, restart_policy.max_retries, e
                        );
                        return Err(e);
                    }
                    
                    retry_count += 1;
                    warn!(
                        "Failed to start MCP server '{}', retrying in {} seconds (attempt {}/{}): {}",
                        name, delay, retry_count, restart_policy.max_retries, e
                    );
                    
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    
                    // Calculate next delay based on backoff strategy
                    delay = match restart_policy.backoff {
                        BackoffStrategy::Fixed => delay,
                        BackoffStrategy::Linear => {
                            (delay + restart_policy.initial_delay).min(restart_policy.max_delay)
                        }
                        BackoffStrategy::Exponential => {
                            (delay * 2).min(restart_policy.max_delay)
                        }
                    };
                }
            }
        }
    }
    
    /// Start a specific MCP server
    async fn start_server(&mut self, name: String, config: McpServerConfig) -> Result<()> {
        info!("Starting MCP server: {}", name);
        
        // Check if already running
        if self.servers.read().await.contains_key(&name) {
            warn!("MCP server '{}' is already running", name);
            return Ok(());
        }
        
        // Create and initialize client
        let mut client = McpClient::new(name.clone(), config.clone()).await?;
        client.initialize().await?;
        
        // Discover tools
        let tools = client.list_tools().await?;
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        
        info!(
            "MCP server '{}' started with {} tools",
            name,
            tool_names.len()
        );
        
        // Register tools with the tool registry (following reference project pattern)
        let client_arc = Arc::new(RwLock::new(client));
        for tool in &tools {
            // Use simple tool name without server prefix for better compatibility
            let wrapper = McpToolWrapper::new(
                tool.clone(),
                client_arc.clone(),
                name.clone(),
            );
            
            // Register with tool registry
            self.tool_registry.register(wrapper)?;
            debug!("Registered MCP tool: {} from server {}", tool.name, name);
        }
        
        // Start health monitoring if configured
        let health_monitor = if let Some(health_config) = config.health_check() {
            let interval = tokio::time::Duration::from_secs(health_config.interval);
            let max_failures = health_config.max_failures;
            
            let name_clone = name.clone();
            let client_clone = client_arc.clone();
            let servers = self.servers.clone();
            
            Some(tokio::spawn(async move {
                let mut consecutive_failures = 0;
                
                loop {
                    tokio::time::sleep(interval).await;
                    
                    // Check if server is still connected
                    let connected = {
                        let client = client_clone.read().await;
                        client.is_connected().await
                    };
                    
                    if connected {
                        consecutive_failures = 0;
                        debug!("Health check passed for MCP server '{}'", name_clone);
                    } else {
                        consecutive_failures += 1;
                        warn!(
                            "Health check failed for MCP server '{}' ({}/{})",
                            name_clone, consecutive_failures, max_failures
                        );
                        
                        if consecutive_failures >= max_failures {
                            error!(
                                "MCP server '{}' failed {} consecutive health checks, marking as failed",
                                name_clone, max_failures
                            );
                            
                            // Remove from active servers
                            let mut servers_guard = servers.write().await;
                            servers_guard.remove(&name_clone);
                            
                            // Note: In a production system, we might want to trigger restart here
                            break;
                        }
                    }
                }
            }))
        } else {
            None
        };
        
        // Store server handle
        let handle = McpServerHandle {
            name: name.clone(),
            client: client_arc,
            tools: tool_names,
            health_monitor,
            restart_count: 0,
        };
        
        self.servers.write().await.insert(name, handle);
        
        Ok(())
    }
    
    /// Stop a specific MCP server
    pub async fn stop_server(&mut self, name: &str) -> Result<()> {
        info!("Stopping MCP server: {}", name);
        
        let mut servers = self.servers.write().await;
        if let Some(mut handle) = servers.remove(name) {
            // Stop health monitor
            if let Some(monitor) = handle.health_monitor.take() {
                monitor.abort();
            }
            
            // Close client connection
            let mut client = handle.client.write().await;
            if let Err(e) = client.close().await {
                warn!("Error closing MCP client '{}': {}", name, e);
            }
            
            // TODO: Unregister tools when ToolRegistry supports it
            // Currently ToolRegistry doesn't have unregister method
            
            info!("MCP server '{}' stopped", name);
        } else {
            warn!("MCP server '{}' not found", name);
        }
        
        Ok(())
    }
    
    /// Stop all MCP servers
    pub async fn stop_all(&mut self) -> Result<()> {
        info!("Stopping all MCP servers");
        
        let server_names: Vec<String> = {
            let servers = self.servers.read().await;
            servers.keys().cloned().collect()
        };
        
        for name in server_names {
            self.stop_server(&name).await?;
        }
        
        Ok(())
    }
    
    /// List running MCP servers
    pub async fn list_servers(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }
    
    /// Get information about a specific server
    pub async fn get_server_info(&self, name: &str) -> Option<(Vec<String>, bool)> {
        let servers = self.servers.read().await;
        if let Some(handle) = servers.get(name) {
            let connected = {
                let client = handle.client.read().await;
                client.is_connected().await
            };
            Some((handle.tools.clone(), connected))
        } else {
            None
        }
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        // Stop all servers when manager is dropped
        // Note: We can't do async cleanup in Drop, so we just log
        // The servers will be cleaned up when their handles are dropped
        debug!("McpManager being dropped, MCP servers will be cleaned up");
    }
}

// Clone implementation for spawning cleanup task
impl Clone for McpManager {
    fn clone(&self) -> Self {
        Self {
            servers: self.servers.clone(),
            tool_registry: self.tool_registry.clone(),
            config: self.config.clone(),
        }
    }
}
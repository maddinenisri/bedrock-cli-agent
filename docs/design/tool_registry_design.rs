// Tool Registry Design with Discovery and Lifecycle Management
// This file contains the design for an advanced tool registry with discovery, validation, and lifecycle management

use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, broadcast, watch};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, Schema};
use bedrock_core::{Result, BedrockError};

/// Tool identifier combining server and tool name
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId {
    pub server_id: String,
    pub tool_name: String,
}

impl ToolId {
    pub fn new(server_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            tool_name: tool_name.into(),
        }
    }
    
    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.server_id, self.tool_name)
    }
}

/// Tool lifecycle state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolState {
    /// Tool is being discovered
    Discovering,
    /// Tool is available and ready for use
    Available,
    /// Tool is temporarily unavailable
    Unavailable,
    /// Tool is deprecated and should not be used
    Deprecated,
    /// Tool has been removed
    Removed,
    /// Tool validation failed
    ValidationFailed,
}

/// Tool metadata for enhanced discovery and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Tool version
    pub version: String,
    /// Tool author/provider
    pub author: Option<String>,
    /// Tool tags for categorization
    pub tags: Vec<String>,
    /// Tool capabilities/features
    pub capabilities: Vec<String>,
    /// Minimum required MCP protocol version
    pub min_protocol_version: String,
    /// Tool deprecation information
    pub deprecation: Option<DeprecationInfo>,
    /// Custom metadata
    pub custom: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecationInfo {
    pub deprecated_since: String,
    pub removal_date: Option<String>,
    pub replacement: Option<String>,
    pub reason: String,
}

/// Enhanced tool definition with lifecycle and metadata
#[derive(Debug, Clone)]
pub struct EnhancedToolDefinition {
    pub id: ToolId,
    pub name: String,
    pub description: String,
    pub schema: Value,
    pub state: ToolState,
    pub metadata: ToolMetadata,
    pub server_capabilities: ServerCapabilities,
    pub registration_time: Instant,
    pub last_used: Option<Instant>,
    pub usage_count: u64,
    pub validation_result: Option<ValidationResult>,
}

/// Server capabilities that affect tool behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub supports_progress: bool,
    pub supports_cancellation: bool,
    pub supports_streaming: bool,
    pub max_concurrent_tools: Option<usize>,
    pub timeout_ms: Option<u64>,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            supports_progress: false,
            supports_cancellation: false,
            supports_streaming: false,
            max_concurrent_tools: None,
            timeout_ms: None,
        }
    }
}

/// Tool validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub compatibility_score: f64,
    pub validated_at: chrono::DateTime<chrono::Utc>,
}

/// Tool registry events
#[derive(Debug, Clone)]
pub enum ToolRegistryEvent {
    ToolDiscovered {
        tool_id: ToolId,
        definition: EnhancedToolDefinition,
    },
    ToolRegistered {
        tool_id: ToolId,
        definition: EnhancedToolDefinition,
    },
    ToolUnregistered {
        tool_id: ToolId,
        reason: String,
    },
    ToolStateChanged {
        tool_id: ToolId,
        old_state: ToolState,
        new_state: ToolState,
    },
    ToolValidationCompleted {
        tool_id: ToolId,
        result: ValidationResult,
    },
    ServerConnected {
        server_id: String,
        capabilities: ServerCapabilities,
    },
    ServerDisconnected {
        server_id: String,
        reason: String,
    },
}

/// Tool filter for querying
#[derive(Debug, Default)]
pub struct ToolFilter {
    pub server_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub capabilities: Option<Vec<String>>,
    pub states: Option<Vec<ToolState>>,
    pub min_compatibility_score: Option<f64>,
    pub name_pattern: Option<String>,
}

/// Tool registry statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryStatistics {
    pub total_tools: usize,
    pub available_tools: usize,
    pub unavailable_tools: usize,
    pub deprecated_tools: usize,
    pub servers_connected: usize,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub average_execution_time_ms: u64,
    pub most_used_tools: Vec<(ToolId, u64)>,
}

/// Core tool registry trait
#[async_trait]
pub trait ToolRegistry: Send + Sync {
    /// Discover tools from a connected server
    async fn discover_tools(
        &self,
        server_id: &str,
        capabilities: ServerCapabilities,
    ) -> Result<Vec<EnhancedToolDefinition>>;
    
    /// Register a discovered tool
    async fn register_tool(
        &self,
        definition: EnhancedToolDefinition,
    ) -> Result<()>;
    
    /// Unregister a tool (when server disconnects)
    async fn unregister_tool(
        &self,
        tool_id: &ToolId,
        reason: &str,
    ) -> Result<()>;
    
    /// Get a tool by its ID
    async fn get_tool(
        &self,
        tool_id: &ToolId,
    ) -> Result<Option<EnhancedToolDefinition>>;
    
    /// Get all tools matching a filter
    async fn query_tools(
        &self,
        filter: ToolFilter,
    ) -> Result<Vec<EnhancedToolDefinition>>;
    
    /// Update tool state
    async fn update_tool_state(
        &self,
        tool_id: &ToolId,
        new_state: ToolState,
    ) -> Result<()>;
    
    /// Validate a tool definition
    async fn validate_tool(
        &self,
        definition: &EnhancedToolDefinition,
    ) -> Result<ValidationResult>;
    
    /// Get registry statistics
    async fn get_statistics(&self) -> Result<RegistryStatistics>;
    
    /// Subscribe to registry events
    async fn subscribe_to_events(
        &self,
    ) -> Result<broadcast::Receiver<ToolRegistryEvent>>;
    
    /// Check tool compatibility with current system
    async fn check_compatibility(
        &self,
        tool_id: &ToolId,
    ) -> Result<f64>;
    
    /// Get tool usage analytics
    async fn get_tool_analytics(
        &self,
        tool_id: &ToolId,
    ) -> Result<ToolAnalytics>;
}

/// Tool usage analytics
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolAnalytics {
    pub tool_id: ToolId,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub average_execution_time_ms: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub error_patterns: Vec<ErrorPattern>,
    pub performance_trend: PerformanceTrend,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub error_type: String,
    pub count: u64,
    pub last_occurrence: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceTrend {
    pub trend_direction: TrendDirection,
    pub average_change_percent: f64,
    pub sample_size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
    Unknown,
}

/// Tool validator trait for extensible validation
#[async_trait]
pub trait ToolValidator: Send + Sync {
    async fn validate(
        &self,
        definition: &EnhancedToolDefinition,
    ) -> Result<ValidationResult>;
    
    fn validator_name(&self) -> &str;
    fn priority(&self) -> i32; // Higher values run first
}

/// Tool discovery service trait
#[async_trait]
pub trait ToolDiscoveryService: Send + Sync {
    async fn discover_from_server(
        &self,
        server_id: &str,
        discovery_config: DiscoveryConfiguration,
    ) -> Result<Vec<EnhancedToolDefinition>>;
    
    async fn validate_server_compatibility(
        &self,
        server_id: &str,
        capabilities: &ServerCapabilities,
    ) -> Result<bool>;
}

/// Discovery configuration
#[derive(Debug, Clone)]
pub struct DiscoveryConfiguration {
    pub include_deprecated: bool,
    pub validate_during_discovery: bool,
    pub discovery_timeout: Duration,
    pub required_capabilities: Vec<String>,
    pub excluded_tags: Vec<String>,
}

impl Default for DiscoveryConfiguration {
    fn default() -> Self {
        Self {
            include_deprecated: false,
            validate_during_discovery: true,
            discovery_timeout: Duration::from_secs(30),
            required_capabilities: vec![],
            excluded_tags: vec![],
        }
    }
}

/// Enhanced tool registry implementation
pub struct EnhancedToolRegistryImpl {
    /// Registered tools indexed by ToolId
    tools: Arc<RwLock<HashMap<ToolId, EnhancedToolDefinition>>>,
    /// Server capabilities indexed by server ID
    server_capabilities: Arc<RwLock<HashMap<String, ServerCapabilities>>>,
    /// Tool validators
    validators: Arc<RwLock<Vec<Arc<dyn ToolValidator>>>>,
    /// Discovery service
    discovery_service: Arc<dyn ToolDiscoveryService>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ToolRegistryEvent>,
    /// Registry configuration
    config: RegistryConfiguration,
    /// Tool analytics
    analytics: Arc<RwLock<HashMap<ToolId, ToolAnalytics>>>,
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

/// Registry configuration
#[derive(Debug, Clone)]
pub struct RegistryConfiguration {
    pub enable_analytics: bool,
    pub cleanup_interval: Duration,
    pub deprecated_tool_retention: Duration,
    pub validation_cache_duration: Duration,
    pub max_concurrent_validations: usize,
}

impl Default for RegistryConfiguration {
    fn default() -> Self {
        Self {
            enable_analytics: true,
            cleanup_interval: Duration::from_secs(3600), // 1 hour
            deprecated_tool_retention: Duration::from_secs(86400 * 30), // 30 days
            validation_cache_duration: Duration::from_secs(3600), // 1 hour
            max_concurrent_validations: 10,
        }
    }
}

impl EnhancedToolRegistryImpl {
    pub fn new(
        discovery_service: Arc<dyn ToolDiscoveryService>,
        config: RegistryConfiguration,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        let registry = Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            server_capabilities: Arc::new(RwLock::new(HashMap::new())),
            validators: Arc::new(RwLock::new(Vec::new())),
            discovery_service,
            event_sender,
            config,
            analytics: Arc::new(RwLock::new(HashMap::new())),
            task_handles: Vec::new(),
        };
        
        // Start background tasks would be implemented here
        registry
    }
    
    /// Add a tool validator
    pub async fn add_validator(&self, validator: Arc<dyn ToolValidator>) -> Result<()> {
        let mut validators = self.validators.write().await;
        validators.push(validator);
        validators.sort_by(|a, b| b.priority().cmp(&a.priority()));
        Ok(())
    }
    
    /// Remove tools from a disconnected server
    pub async fn handle_server_disconnect(&self, server_id: &str) -> Result<()> {
        let mut tools = self.tools.write().await;
        let mut removed_tools = Vec::new();
        
        // Find tools from this server
        for (tool_id, _) in tools.iter() {
            if tool_id.server_id == server_id {
                removed_tools.push(tool_id.clone());
            }
        }
        
        // Remove tools and emit events
        for tool_id in removed_tools {
            if tools.remove(&tool_id).is_some() {
                let _ = self.event_sender.send(ToolRegistryEvent::ToolUnregistered {
                    tool_id,
                    reason: "Server disconnected".to_string(),
                });
            }
        }
        
        // Remove server capabilities
        let mut capabilities = self.server_capabilities.write().await;
        capabilities.remove(server_id);
        
        let _ = self.event_sender.send(ToolRegistryEvent::ServerDisconnected {
            server_id: server_id.to_string(),
            reason: "Connection lost".to_string(),
        });
        
        Ok(())
    }
    
    /// Internal validation with all registered validators
    async fn internal_validate(
        &self,
        definition: &EnhancedToolDefinition,
    ) -> Result<ValidationResult> {
        let validators = self.validators.read().await;
        let mut all_errors = Vec::new();
        let mut all_warnings = Vec::new();
        let mut total_score = 0.0;
        let mut validator_count = 0;
        
        for validator in validators.iter() {
            match validator.validate(definition).await {
                Ok(result) => {
                    all_errors.extend(result.errors);
                    all_warnings.extend(result.warnings);
                    total_score += result.compatibility_score;
                    validator_count += 1;
                }
                Err(e) => {
                    all_errors.push(format!("Validator {} failed: {}", validator.validator_name(), e));
                }
            }
        }
        
        let average_score = if validator_count > 0 {
            total_score / validator_count as f64
        } else {
            0.0
        };
        
        Ok(ValidationResult {
            is_valid: all_errors.is_empty(),
            errors: all_errors,
            warnings: all_warnings,
            compatibility_score: average_score,
            validated_at: chrono::Utc::now(),
        })
    }
    
    /// Update tool analytics
    async fn update_analytics(
        &self,
        tool_id: &ToolId,
        execution_time_ms: u64,
        success: bool,
    ) -> Result<()> {
        if !self.config.enable_analytics {
            return Ok(());
        }
        
        let mut analytics = self.analytics.write().await;
        let tool_analytics = analytics.entry(tool_id.clone()).or_insert_with(|| {
            ToolAnalytics {
                tool_id: tool_id.clone(),
                total_executions: 0,
                successful_executions: 0,
                failed_executions: 0,
                average_execution_time_ms: 0,
                last_execution: None,
                error_patterns: vec![],
                performance_trend: PerformanceTrend {
                    trend_direction: TrendDirection::Unknown,
                    average_change_percent: 0.0,
                    sample_size: 0,
                },
            }
        });
        
        tool_analytics.total_executions += 1;
        if success {
            tool_analytics.successful_executions += 1;
        } else {
            tool_analytics.failed_executions += 1;
        }
        
        // Update average execution time
        let current_avg = tool_analytics.average_execution_time_ms;
        let total = tool_analytics.total_executions;
        tool_analytics.average_execution_time_ms = 
            ((current_avg * (total - 1)) + execution_time_ms) / total;
        
        tool_analytics.last_execution = Some(chrono::Utc::now());
        
        Ok(())
    }
}

#[async_trait]
impl ToolRegistry for EnhancedToolRegistryImpl {
    async fn discover_tools(
        &self,
        server_id: &str,
        capabilities: ServerCapabilities,
    ) -> Result<Vec<EnhancedToolDefinition>> {
        // Store server capabilities
        {
            let mut server_caps = self.server_capabilities.write().await;
            server_caps.insert(server_id.to_string(), capabilities.clone());
        }
        
        // Emit server connected event
        let _ = self.event_sender.send(ToolRegistryEvent::ServerConnected {
            server_id: server_id.to_string(),
            capabilities: capabilities.clone(),
        });
        
        // Discover tools from server
        let discovery_config = DiscoveryConfiguration::default();
        let discovered_tools = self.discovery_service
            .discover_from_server(server_id, discovery_config)
            .await?;
        
        // Validate tools if configured
        let mut validated_tools = Vec::new();
        for tool in discovered_tools {
            let _ = self.event_sender.send(ToolRegistryEvent::ToolDiscovered {
                tool_id: tool.id.clone(),
                definition: tool.clone(),
            });
            
            let validation_result = self.internal_validate(&tool).await?;
            let mut enhanced_tool = tool;
            enhanced_tool.validation_result = Some(validation_result.clone());
            
            if validation_result.is_valid {
                enhanced_tool.state = ToolState::Available;
            } else {
                enhanced_tool.state = ToolState::ValidationFailed;
            }
            
            let _ = self.event_sender.send(ToolRegistryEvent::ToolValidationCompleted {
                tool_id: enhanced_tool.id.clone(),
                result: validation_result,
            });
            
            validated_tools.push(enhanced_tool);
        }
        
        Ok(validated_tools)
    }
    
    async fn register_tool(&self, definition: EnhancedToolDefinition) -> Result<()> {
        let tool_id = definition.id.clone();
        
        // Check for conflicts
        {
            let tools = self.tools.read().await;
            if tools.contains_key(&tool_id) {
                return Err(BedrockError::ConfigError(
                    format!("Tool {} already registered", tool_id.qualified_name())
                ));
            }
        }
        
        // Register the tool
        {
            let mut tools = self.tools.write().await;
            tools.insert(tool_id.clone(), definition.clone());
        }
        
        // Emit registration event
        let _ = self.event_sender.send(ToolRegistryEvent::ToolRegistered {
            tool_id,
            definition,
        });
        
        Ok(())
    }
    
    async fn unregister_tool(&self, tool_id: &ToolId, reason: &str) -> Result<()> {
        let mut tools = self.tools.write().await;
        if tools.remove(tool_id).is_some() {
            let _ = self.event_sender.send(ToolRegistryEvent::ToolUnregistered {
                tool_id: tool_id.clone(),
                reason: reason.to_string(),
            });
        }
        Ok(())
    }
    
    async fn get_tool(&self, tool_id: &ToolId) -> Result<Option<EnhancedToolDefinition>> {
        let tools = self.tools.read().await;
        Ok(tools.get(tool_id).cloned())
    }
    
    async fn query_tools(&self, filter: ToolFilter) -> Result<Vec<EnhancedToolDefinition>> {
        let tools = self.tools.read().await;
        let mut results = Vec::new();
        
        for (tool_id, definition) in tools.iter() {
            // Apply filters
            if let Some(ref server_filter) = filter.server_id {
                if tool_id.server_id != *server_filter {
                    continue;
                }
            }
            
            if let Some(ref states_filter) = filter.states {
                if !states_filter.contains(&definition.state) {
                    continue;
                }
            }
            
            if let Some(ref tags_filter) = filter.tags {
                let has_required_tags = tags_filter.iter()
                    .all(|tag| definition.metadata.tags.contains(tag));
                if !has_required_tags {
                    continue;
                }
            }
            
            if let Some(min_score) = filter.min_compatibility_score {
                if let Some(ref validation) = definition.validation_result {
                    if validation.compatibility_score < min_score {
                        continue;
                    }
                }
            }
            
            if let Some(ref pattern) = filter.name_pattern {
                if !definition.name.contains(pattern) {
                    continue;
                }
            }
            
            results.push(definition.clone());
        }
        
        Ok(results)
    }
    
    async fn update_tool_state(&self, tool_id: &ToolId, new_state: ToolState) -> Result<()> {
        let mut tools = self.tools.write().await;
        if let Some(tool) = tools.get_mut(tool_id) {
            let old_state = tool.state.clone();
            tool.state = new_state.clone();
            
            let _ = self.event_sender.send(ToolRegistryEvent::ToolStateChanged {
                tool_id: tool_id.clone(),
                old_state,
                new_state,
            });
        }
        Ok(())
    }
    
    async fn validate_tool(&self, definition: &EnhancedToolDefinition) -> Result<ValidationResult> {
        self.internal_validate(definition).await
    }
    
    async fn get_statistics(&self) -> Result<RegistryStatistics> {
        let tools = self.tools.read().await;
        let analytics = self.analytics.read().await;
        let server_capabilities = self.server_capabilities.read().await;
        
        let mut stats = RegistryStatistics {
            total_tools: tools.len(),
            available_tools: 0,
            unavailable_tools: 0,
            deprecated_tools: 0,
            servers_connected: server_capabilities.len(),
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_execution_time_ms: 0,
            most_used_tools: vec![],
        };
        
        // Count tools by state
        for definition in tools.values() {
            match definition.state {
                ToolState::Available => stats.available_tools += 1,
                ToolState::Unavailable | ToolState::ValidationFailed => stats.unavailable_tools += 1,
                ToolState::Deprecated => stats.deprecated_tools += 1,
                _ => {}
            }
        }
        
        // Aggregate analytics
        let mut usage_counts: Vec<(ToolId, u64)> = vec![];
        for (tool_id, tool_analytics) in analytics.iter() {
            stats.total_executions += tool_analytics.total_executions;
            stats.successful_executions += tool_analytics.successful_executions;
            stats.failed_executions += tool_analytics.failed_executions;
            usage_counts.push((tool_id.clone(), tool_analytics.total_executions));
        }
        
        // Sort and take top 10 most used tools
        usage_counts.sort_by(|a, b| b.1.cmp(&a.1));
        stats.most_used_tools = usage_counts.into_iter().take(10).collect();
        
        // Calculate average execution time
        if !analytics.is_empty() {
            let total_avg: u64 = analytics.values()
                .map(|a| a.average_execution_time_ms)
                .sum();
            stats.average_execution_time_ms = total_avg / analytics.len() as u64;
        }
        
        Ok(stats)
    }
    
    async fn subscribe_to_events(&self) -> Result<broadcast::Receiver<ToolRegistryEvent>> {
        Ok(self.event_sender.subscribe())
    }
    
    async fn check_compatibility(&self, tool_id: &ToolId) -> Result<f64> {
        let tools = self.tools.read().await;
        if let Some(tool) = tools.get(tool_id) {
            if let Some(ref validation) = tool.validation_result {
                Ok(validation.compatibility_score)
            } else {
                Ok(0.0)
            }
        } else {
            Err(BedrockError::McpError(format!("Tool {} not found", tool_id.qualified_name())))
        }
    }
    
    async fn get_tool_analytics(&self, tool_id: &ToolId) -> Result<ToolAnalytics> {
        let analytics = self.analytics.read().await;
        analytics.get(tool_id)
            .cloned()
            .ok_or_else(|| BedrockError::McpError(format!("No analytics found for tool {}", tool_id.qualified_name())))
    }
}

// Example validators
#[derive(Debug)]
pub struct SchemaValidator;

#[async_trait]
impl ToolValidator for SchemaValidator {
    async fn validate(&self, definition: &EnhancedToolDefinition) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut score = 1.0;
        
        // Basic schema validation
        if definition.schema.is_null() {
            errors.push("Tool schema is null".to_string());
            score = 0.0;
        } else {
            // Validate JSON schema structure
            if !definition.schema.is_object() {
                warnings.push("Tool schema should be an object".to_string());
                score *= 0.8;
            }
        }
        
        // Description validation
        if definition.description.trim().is_empty() {
            warnings.push("Tool description is empty".to_string());
            score *= 0.9;
        }
        
        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            compatibility_score: score,
            validated_at: chrono::Utc::now(),
        })
    }
    
    fn validator_name(&self) -> &str {
        "SchemaValidator"
    }
    
    fn priority(&self) -> i32 {
        100 // High priority
    }
}

#[derive(Debug)]
pub struct CompatibilityValidator;

#[async_trait]
impl ToolValidator for CompatibilityValidator {
    async fn validate(&self, definition: &EnhancedToolDefinition) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut score = 1.0;
        
        // Check protocol version compatibility
        let min_version = &definition.metadata.min_protocol_version;
        if min_version > "2024-11-05" {
            errors.push(format!("Tool requires unsupported protocol version: {}", min_version));
            score = 0.0;
        }
        
        // Check required capabilities
        for capability in &definition.metadata.capabilities {
            if capability == "streaming" && !definition.server_capabilities.supports_streaming {
                warnings.push(format!("Tool requires streaming but server doesn't support it"));
                score *= 0.7;
            }
        }
        
        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            compatibility_score: score,
            validated_at: chrono::Utc::now(),
        })
    }
    
    fn validator_name(&self) -> &str {
        "CompatibilityValidator"
    }
    
    fn priority(&self) -> i32 {
        50 // Medium priority
    }
}
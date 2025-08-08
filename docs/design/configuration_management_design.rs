// Configuration Management Design with Hot-Reload Capability
// This file contains the design for dynamic configuration management with hot-reload support

use async_trait::async_trait;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{RwLock, broadcast, watch};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;
use bedrock_core::{Result, BedrockError};

/// Configuration change event
#[derive(Debug, Clone)]
pub enum ConfigurationEvent {
    /// Configuration file was added
    FileAdded {
        path: PathBuf,
        config_type: String,
    },
    /// Configuration file was modified
    FileModified {
        path: PathBuf,
        config_type: String,
        old_hash: String,
        new_hash: String,
    },
    /// Configuration file was deleted
    FileDeleted {
        path: PathBuf,
        config_type: String,
    },
    /// Configuration was reloaded
    ConfigurationReloaded {
        config_type: String,
        changes: Vec<ConfigurationChange>,
    },
    /// Configuration validation failed
    ValidationFailed {
        path: PathBuf,
        errors: Vec<String>,
    },
    /// Configuration merge conflict occurred
    MergeConflict {
        source1: String,
        source2: String,
        conflicts: Vec<String>,
    },
}

/// Specific configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationChange {
    pub path: String,
    pub change_type: ChangeType,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

/// Configuration source types
#[derive(Debug, Clone)]
pub enum ConfigurationSource {
    /// File-based configuration
    File {
        path: PathBuf,
        format: ConfigurationFormat,
        watch: bool,
    },
    /// Directory-based configuration (watches all files in directory)
    Directory {
        path: PathBuf,
        pattern: String,
        recursive: bool,
        format: ConfigurationFormat,
    },
    /// Environment variables
    Environment {
        prefix: String,
        separator: String,
    },
    /// Remote configuration (HTTP endpoint)
    Remote {
        url: String,
        headers: HashMap<String, String>,
        poll_interval: Duration,
        format: ConfigurationFormat,
    },
    /// In-memory configuration (for testing)
    Memory {
        data: serde_json::Value,
    },
}

/// Supported configuration formats
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationFormat {
    Yaml,
    Json,
    Toml,
    Properties,
}

impl ConfigurationFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "yaml" | "yml" => Some(Self::Yaml),
            "json" => Some(Self::Json),
            "toml" => Some(Self::Toml),
            "properties" | "props" => Some(Self::Properties),
            _ => None,
        }
    }
}

/// Configuration validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub error_code: String,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub path: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Configuration merge strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Override: Later sources completely replace earlier ones
    Override,
    /// Merge: Deep merge objects, arrays append
    DeepMerge,
    /// Selective: Only merge specified paths
    Selective(Vec<String>),
    /// Custom: Use custom merge function
    Custom,
}

/// Configuration metadata
#[derive(Debug, Clone)]
pub struct ConfigurationMetadata {
    pub source: String,
    pub loaded_at: chrono::DateTime<chrono::Utc>,
    pub hash: String,
    pub size: u64,
    pub format: ConfigurationFormat,
    pub validation_result: Option<ValidationResult>,
}

/// Configuration snapshot for rollback
#[derive(Debug, Clone)]
pub struct ConfigurationSnapshot {
    pub id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub config: serde_json::Value,
    pub metadata: HashMap<String, ConfigurationMetadata>,
    pub description: String,
}

/// Configuration change handler trait
#[async_trait]
pub trait ConfigurationChangeHandler: Send + Sync {
    async fn handle_change(
        &self,
        change_type: &str,
        old_config: Option<&serde_json::Value>,
        new_config: &serde_json::Value,
    ) -> Result<()>;
    
    fn handler_name(&self) -> &str;
    fn interested_paths(&self) -> Vec<String>;
}

/// Configuration validator trait
#[async_trait]
pub trait ConfigurationValidator: Send + Sync {
    async fn validate(&self, config: &serde_json::Value) -> Result<ValidationResult>;
    fn validator_name(&self) -> &str;
    fn config_type(&self) -> &str;
}

/// Main configuration manager trait
#[async_trait]
pub trait ConfigurationManager: Send + Sync {
    /// Load configuration from multiple sources
    async fn load_configuration(
        &self,
        sources: Vec<ConfigurationSource>,
        merge_strategy: MergeStrategy,
    ) -> Result<serde_json::Value>;
    
    /// Get current configuration
    async fn get_configuration(&self) -> Result<serde_json::Value>;
    
    /// Get configuration section by path
    async fn get_configuration_section(&self, path: &str) -> Result<Option<serde_json::Value>>;
    
    /// Update configuration programmatically
    async fn update_configuration(
        &self,
        path: &str,
        value: serde_json::Value,
    ) -> Result<()>;
    
    /// Reload configuration from all sources
    async fn reload_configuration(&self) -> Result<()>;
    
    /// Watch for configuration changes
    async fn start_watching(&self) -> Result<()>;
    
    /// Stop watching for configuration changes
    async fn stop_watching(&self) -> Result<()>;
    
    /// Subscribe to configuration change events
    async fn subscribe_to_changes(
        &self,
    ) -> Result<broadcast::Receiver<ConfigurationEvent>>;
    
    /// Register a change handler
    async fn register_change_handler(
        &self,
        handler: Arc<dyn ConfigurationChangeHandler>,
    ) -> Result<()>;
    
    /// Register a validator
    async fn register_validator(
        &self,
        validator: Arc<dyn ConfigurationValidator>,
    ) -> Result<()>;
    
    /// Validate current configuration
    async fn validate_configuration(&self) -> Result<ValidationResult>;
    
    /// Create a configuration snapshot
    async fn create_snapshot(&self, description: String) -> Result<ConfigurationSnapshot>;
    
    /// Restore from a configuration snapshot
    async fn restore_snapshot(&self, snapshot_id: Uuid) -> Result<()>;
    
    /// Get configuration history
    async fn get_snapshots(&self) -> Result<Vec<ConfigurationSnapshot>>;
    
    /// Get configuration statistics
    async fn get_statistics(&self) -> Result<ConfigurationStatistics>;
}

/// Configuration statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigurationStatistics {
    pub total_sources: usize,
    pub active_watchers: usize,
    pub total_reloads: u64,
    pub successful_reloads: u64,
    pub failed_reloads: u64,
    pub last_reload: Option<chrono::DateTime<chrono::Utc>>,
    pub configuration_size: u64,
    pub snapshots_count: usize,
    pub validation_errors: u64,
    pub change_handlers: usize,
    pub validators: usize,
}

/// Configuration manager implementation
pub struct ConfigurationManagerImpl {
    /// Current configuration
    current_config: Arc<RwLock<serde_json::Value>>,
    /// Configuration metadata for each source
    metadata: Arc<RwLock<HashMap<String, ConfigurationMetadata>>>,
    /// Configuration sources
    sources: Arc<RwLock<Vec<ConfigurationSource>>>,
    /// Merge strategy
    merge_strategy: Arc<RwLock<MergeStrategy>>,
    /// File watchers
    watchers: Arc<RwLock<HashMap<String, FileWatcher>>>,
    /// Change handlers
    change_handlers: Arc<RwLock<Vec<Arc<dyn ConfigurationChangeHandler>>>>,
    /// Validators
    validators: Arc<RwLock<Vec<Arc<dyn ConfigurationValidator>>>>,
    /// Configuration snapshots
    snapshots: Arc<RwLock<Vec<ConfigurationSnapshot>>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ConfigurationEvent>,
    /// Configuration for the manager itself
    manager_config: ConfigurationManagerConfig,
    /// Statistics
    statistics: Arc<RwLock<ConfigurationStatistics>>,
    /// Background task handles
    task_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Manager configuration
#[derive(Debug, Clone)]
pub struct ConfigurationManagerConfig {
    pub enable_hot_reload: bool,
    pub reload_debounce_duration: Duration,
    pub max_snapshots: usize,
    pub enable_validation: bool,
    pub enable_change_tracking: bool,
    pub backup_on_change: bool,
    pub validate_on_load: bool,
}

impl Default for ConfigurationManagerConfig {
    fn default() -> Self {
        Self {
            enable_hot_reload: true,
            reload_debounce_duration: Duration::from_millis(500),
            max_snapshots: 50,
            enable_validation: true,
            enable_change_tracking: true,
            backup_on_change: true,
            validate_on_load: true,
        }
    }
}

/// File watcher for monitoring configuration changes
pub struct FileWatcher {
    pub path: PathBuf,
    pub last_modified: std::time::SystemTime,
    pub last_hash: String,
    pub handle: Option<tokio::task::JoinHandle<()>>,
}

impl ConfigurationManagerImpl {
    pub fn new(config: ConfigurationManagerConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        
        Self {
            current_config: Arc::new(RwLock::new(serde_json::Value::Null)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            sources: Arc::new(RwLock::new(Vec::new())),
            merge_strategy: Arc::new(RwLock::new(MergeStrategy::DeepMerge)),
            watchers: Arc::new(RwLock::new(HashMap::new())),
            change_handlers: Arc::new(RwLock::new(Vec::new())),
            validators: Arc::new(RwLock::new(Vec::new())),
            snapshots: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            manager_config: config,
            statistics: Arc::new(RwLock::new(ConfigurationStatistics::default())),
            task_handles: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Load configuration from a specific source
    async fn load_from_source(&self, source: &ConfigurationSource) -> Result<(serde_json::Value, ConfigurationMetadata)> {
        match source {
            ConfigurationSource::File { path, format, .. } => {
                self.load_from_file(path, format).await
            }
            ConfigurationSource::Directory { path, pattern, recursive, format } => {
                self.load_from_directory(path, pattern, *recursive, format).await
            }
            ConfigurationSource::Environment { prefix, separator } => {
                self.load_from_environment(prefix, separator).await
            }
            ConfigurationSource::Remote { url, headers, format, .. } => {
                self.load_from_remote(url, headers, format).await
            }
            ConfigurationSource::Memory { data } => {
                Ok((data.clone(), ConfigurationMetadata {
                    source: "memory".to_string(),
                    loaded_at: chrono::Utc::now(),
                    hash: self.calculate_hash(data)?,
                    size: data.to_string().len() as u64,
                    format: ConfigurationFormat::Json,
                    validation_result: None,
                }))
            }
        }
    }
    
    async fn load_from_file(&self, path: &Path, format: &ConfigurationFormat) -> Result<(serde_json::Value, ConfigurationMetadata)> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| BedrockError::IoError(e))?;
        
        let value = self.parse_configuration(&content, format)?;
        let metadata = std::fs::metadata(path)
            .map_err(|e| BedrockError::IoError(e))?;
        
        Ok((value.clone(), ConfigurationMetadata {
            source: path.to_string_lossy().to_string(),
            loaded_at: chrono::Utc::now(),
            hash: self.calculate_hash(&value)?,
            size: metadata.len(),
            format: format.clone(),
            validation_result: None,
        }))
    }
    
    async fn load_from_directory(
        &self,
        path: &Path,
        pattern: &str,
        recursive: bool,
        format: &ConfigurationFormat,
    ) -> Result<(serde_json::Value, ConfigurationMetadata)> {
        let mut combined_config = serde_json::json!({});
        let mut total_size = 0;
        
        let entries = if recursive {
            self.find_files_recursive(path, pattern).await?
        } else {
            self.find_files_in_directory(path, pattern).await?
        };
        
        for file_path in entries {
            let (config, metadata) = self.load_from_file(&file_path, format).await?;
            combined_config = self.merge_configurations(combined_config, config, &MergeStrategy::DeepMerge)?;
            total_size += metadata.size;
        }
        
        Ok((combined_config.clone(), ConfigurationMetadata {
            source: path.to_string_lossy().to_string(),
            loaded_at: chrono::Utc::now(),
            hash: self.calculate_hash(&combined_config)?,
            size: total_size,
            format: format.clone(),
            validation_result: None,
        }))
    }
    
    async fn load_from_environment(&self, prefix: &str, separator: &str) -> Result<(serde_json::Value, ConfigurationMetadata)> {
        let mut config = serde_json::Map::new();
        
        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let config_key = key.strip_prefix(prefix)
                    .unwrap_or(&key)
                    .replace(separator, ".");
                
                // Convert environment value to appropriate JSON type
                let json_value = if let Ok(int_val) = value.parse::<i64>() {
                    serde_json::Value::Number(serde_json::Number::from(int_val))
                } else if let Ok(float_val) = value.parse::<f64>() {
                    serde_json::Value::Number(serde_json::Number::from_f64(float_val).unwrap())
                } else if let Ok(bool_val) = value.parse::<bool>() {
                    serde_json::Value::Bool(bool_val)
                } else {
                    serde_json::Value::String(value)
                };
                
                self.set_nested_value(&mut config, &config_key, json_value);
            }
        }
        
        let final_config = serde_json::Value::Object(config);
        
        Ok((final_config.clone(), ConfigurationMetadata {
            source: format!("environment:{}", prefix),
            loaded_at: chrono::Utc::now(),
            hash: self.calculate_hash(&final_config)?,
            size: final_config.to_string().len() as u64,
            format: ConfigurationFormat::Json,
            validation_result: None,
        }))
    }
    
    async fn load_from_remote(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        format: &ConfigurationFormat,
    ) -> Result<(serde_json::Value, ConfigurationMetadata)> {
        let client = reqwest::Client::new();
        let mut request = client.get(url);
        
        for (key, value) in headers {
            request = request.header(key, value);
        }
        
        let response = request.send().await
            .map_err(|e| BedrockError::McpError(format!("Failed to fetch remote config: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(BedrockError::McpError(
                format!("Remote configuration fetch failed: {}", response.status())
            ));
        }
        
        let content = response.text().await
            .map_err(|e| BedrockError::McpError(format!("Failed to read remote config: {}", e)))?;
        
        let value = self.parse_configuration(&content, format)?;
        
        Ok((value.clone(), ConfigurationMetadata {
            source: url.to_string(),
            loaded_at: chrono::Utc::now(),
            hash: self.calculate_hash(&value)?,
            size: content.len() as u64,
            format: format.clone(),
            validation_result: None,
        }))
    }
    
    fn parse_configuration(&self, content: &str, format: &ConfigurationFormat) -> Result<serde_json::Value> {
        match format {
            ConfigurationFormat::Json => {
                serde_json::from_str(content)
                    .map_err(|e| BedrockError::SerializationError(e))
            }
            ConfigurationFormat::Yaml => {
                serde_yaml::from_str(content)
                    .map_err(|e| BedrockError::ConfigError(format!("YAML parse error: {}", e)))
            }
            ConfigurationFormat::Toml => {
                toml::from_str::<serde_json::Value>(content)
                    .map_err(|e| BedrockError::ConfigError(format!("TOML parse error: {}", e)))
            }
            ConfigurationFormat::Properties => {
                // Simple properties parser (key=value format)
                let mut map = serde_json::Map::new();
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        map.insert(key.trim().to_string(), serde_json::Value::String(value.trim().to_string()));
                    }
                }
                Ok(serde_json::Value::Object(map))
            }
        }
    }
    
    fn merge_configurations(
        &self,
        base: serde_json::Value,
        overlay: serde_json::Value,
        strategy: &MergeStrategy,
    ) -> Result<serde_json::Value> {
        match strategy {
            MergeStrategy::Override => Ok(overlay),
            MergeStrategy::DeepMerge => {
                Ok(self.deep_merge(base, overlay))
            }
            MergeStrategy::Selective(paths) => {
                let mut result = base;
                for path in paths {
                    if let Some(value) = self.get_nested_value(&overlay, path) {
                        self.set_nested_value_in_json(&mut result, path, value.clone());
                    }
                }
                Ok(result)
            }
            MergeStrategy::Custom => {
                // Would call custom merge function
                Ok(self.deep_merge(base, overlay))
            }
        }
    }
    
    fn deep_merge(&self, mut base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
        match (base, overlay) {
            (serde_json::Value::Object(ref mut base_map), serde_json::Value::Object(overlay_map)) => {
                for (key, value) in overlay_map {
                    if let Some(base_value) = base_map.get_mut(&key) {
                        *base_value = self.deep_merge(base_value.clone(), value);
                    } else {
                        base_map.insert(key, value);
                    }
                }
                base
            }
            (_, overlay) => overlay,
        }
    }
    
    fn calculate_hash(&self, value: &serde_json::Value) -> Result<String> {
        use sha2::{Sha256, Digest};
        let content = serde_json::to_string(value)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }
    
    fn set_nested_value(&self, map: &mut serde_json::Map<String, serde_json::Value>, path: &str, value: serde_json::Value) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = map;
        
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.insert(part.to_string(), value.clone());
            } else {
                let entry = current.entry(part.to_string()).or_insert_with(|| serde_json::json!({}));
                if let serde_json::Value::Object(ref mut obj) = entry {
                    current = obj;
                }
            }
        }
    }
    
    fn get_nested_value(&self, value: &serde_json::Value, path: &str) -> Option<&serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;
        
        for part in parts {
            match current {
                serde_json::Value::Object(map) => {
                    current = map.get(part)?;
                }
                _ => return None,
            }
        }
        
        Some(current)
    }
    
    fn set_nested_value_in_json(&self, value: &mut serde_json::Value, path: &str, new_value: serde_json::Value) {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;
        
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                if let serde_json::Value::Object(ref mut map) = current {
                    map.insert(part.to_string(), new_value.clone());
                }
            } else {
                if let serde_json::Value::Object(ref mut map) = current {
                    let entry = map.entry(part.to_string()).or_insert_with(|| serde_json::json!({}));
                    current = entry;
                }
            }
        }
    }
    
    async fn find_files_in_directory(&self, path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(path).await
            .map_err(|e| BedrockError::IoError(e))?;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| BedrockError::IoError(e))? {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if glob_match(pattern, filename) {
                        files.push(path);
                    }
                }
            }
        }
        
        Ok(files)
    }
    
    async fn find_files_recursive(&self, path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.find_files_recursive_impl(path, pattern, &mut files).await?;
        Ok(files)
    }
    
    fn find_files_recursive_impl<'a>(&'a self, path: &'a Path, pattern: &'a str, files: &'a mut Vec<PathBuf>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(path).await
                .map_err(|e| BedrockError::IoError(e))?;
            
            while let Some(entry) = entries.next_entry().await
                .map_err(|e| BedrockError::IoError(e))? {
                let path = entry.path();
                if path.is_dir() {
                    self.find_files_recursive_impl(&path, pattern, files).await?;
                } else if path.is_file() {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        if glob_match(pattern, filename) {
                            files.push(path);
                        }
                    }
                }
            }
            
            Ok(())
        })
    }
    
    /// Notify change handlers about configuration changes
    async fn notify_change_handlers(
        &self,
        change_type: &str,
        old_config: Option<&serde_json::Value>,
        new_config: &serde_json::Value,
    ) -> Result<()> {
        let handlers = self.change_handlers.read().await;
        
        for handler in handlers.iter() {
            if let Err(e) = handler.handle_change(change_type, old_config, new_config).await {
                tracing::error!("Change handler {} failed: {}", handler.handler_name(), e);
            }
        }
        
        Ok(())
    }
    
    /// Validate configuration using registered validators
    async fn internal_validate(&self, config: &serde_json::Value) -> Result<ValidationResult> {
        let validators = self.validators.read().await;
        let mut all_errors = Vec::new();
        let mut all_warnings = Vec::new();
        
        for validator in validators.iter() {
            match validator.validate(config).await {
                Ok(result) => {
                    all_errors.extend(result.errors);
                    all_warnings.extend(result.warnings);
                }
                Err(e) => {
                    all_errors.push(ValidationError {
                        path: "validator".to_string(),
                        message: format!("Validator {} failed: {}", validator.validator_name(), e),
                        error_code: "VALIDATOR_ERROR".to_string(),
                    });
                }
            }
        }
        
        Ok(ValidationResult {
            is_valid: all_errors.is_empty(),
            errors: all_errors,
            warnings: all_warnings,
        })
    }
}

#[async_trait]
impl ConfigurationManager for ConfigurationManagerImpl {
    async fn load_configuration(
        &self,
        sources: Vec<ConfigurationSource>,
        merge_strategy: MergeStrategy,
    ) -> Result<serde_json::Value> {
        // Store sources and merge strategy
        {
            let mut stored_sources = self.sources.write().await;
            *stored_sources = sources.clone();
        }
        {
            let mut stored_strategy = self.merge_strategy.write().await;
            *stored_strategy = merge_strategy.clone();
        }
        
        // Load from all sources
        let mut merged_config = serde_json::json!({});
        let mut all_metadata = HashMap::new();
        
        for source in &sources {
            match self.load_from_source(source).await {
                Ok((config, metadata)) => {
                    let source_key = metadata.source.clone();
                    merged_config = self.merge_configurations(merged_config, config, &merge_strategy)?;
                    all_metadata.insert(source_key, metadata);
                }
                Err(e) => {
                    tracing::error!("Failed to load configuration from source: {}", e);
                    // Continue loading other sources
                }
            }
        }
        
        // Validate if enabled
        if self.manager_config.validate_on_load {
            let validation_result = self.internal_validate(&merged_config).await?;
            if !validation_result.is_valid {
                return Err(BedrockError::ConfigError(
                    format!("Configuration validation failed: {:?}", validation_result.errors)
                ));
            }
        }
        
        // Store current configuration
        {
            let mut current_config = self.current_config.write().await;
            *current_config = merged_config.clone();
        }
        {
            let mut metadata = self.metadata.write().await;
            *metadata = all_metadata;
        }
        
        // Update statistics
        {
            let mut stats = self.statistics.write().await;
            stats.total_sources = sources.len();
            stats.total_reloads += 1;
            stats.successful_reloads += 1;
            stats.last_reload = Some(chrono::Utc::now());
            stats.configuration_size = merged_config.to_string().len() as u64;
        }
        
        Ok(merged_config)
    }
    
    async fn get_configuration(&self) -> Result<serde_json::Value> {
        let config = self.current_config.read().await;
        Ok(config.clone())
    }
    
    async fn get_configuration_section(&self, path: &str) -> Result<Option<serde_json::Value>> {
        let config = self.current_config.read().await;
        Ok(self.get_nested_value(&config, path).cloned())
    }
    
    async fn update_configuration(&self, path: &str, value: serde_json::Value) -> Result<()> {
        let old_config = {
            let config = self.current_config.read().await;
            config.clone()
        };
        
        let mut new_config = old_config.clone();
        self.set_nested_value_in_json(&mut new_config, path, value);
        
        // Validate new configuration
        if self.manager_config.enable_validation {
            let validation_result = self.internal_validate(&new_config).await?;
            if !validation_result.is_valid {
                return Err(BedrockError::ConfigError(
                    format!("Configuration validation failed: {:?}", validation_result.errors)
                ));
            }
        }
        
        // Create snapshot if enabled
        if self.manager_config.backup_on_change {
            let snapshot = ConfigurationSnapshot {
                id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                config: old_config.clone(),
                metadata: {
                    let metadata = self.metadata.read().await;
                    metadata.clone()
                },
                description: format!("Before updating path: {}", path),
            };
            
            let mut snapshots = self.snapshots.write().await;
            snapshots.push(snapshot);
            
            // Limit snapshots
            if snapshots.len() > self.manager_config.max_snapshots {
                snapshots.remove(0);
            }
        }
        
        // Update configuration
        {
            let mut current_config = self.current_config.write().await;
            *current_config = new_config.clone();
        }
        
        // Notify change handlers
        self.notify_change_handlers("update", Some(&old_config), &new_config).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConfigurationEvent::ConfigurationReloaded {
            config_type: "programmatic_update".to_string(),
            changes: vec![ConfigurationChange {
                path: path.to_string(),
                change_type: ChangeType::Modified,
                old_value: self.get_nested_value(&old_config, path).cloned(),
                new_value: self.get_nested_value(&new_config, path).cloned(),
            }],
        });
        
        Ok(())
    }
    
    async fn reload_configuration(&self) -> Result<()> {
        let sources = {
            let stored_sources = self.sources.read().await;
            stored_sources.clone()
        };
        
        let merge_strategy = {
            let stored_strategy = self.merge_strategy.read().await;
            stored_strategy.clone()
        };
        
        self.load_configuration(sources, merge_strategy).await?;
        Ok(())
    }
    
    async fn start_watching(&self) -> Result<()> {
        if !self.manager_config.enable_hot_reload {
            return Ok(());
        }
        
        let sources = {
            let stored_sources = self.sources.read().await;
            stored_sources.clone()
        };
        
        // Start file watchers for file-based sources
        for source in sources {
            match source {
                ConfigurationSource::File { path, watch: true, .. } => {
                    self.start_file_watcher(path).await?;
                }
                ConfigurationSource::Directory { path, .. } => {
                    self.start_directory_watcher(path).await?;
                }
                ConfigurationSource::Remote { poll_interval, .. } => {
                    self.start_remote_watcher(source, poll_interval).await?;
                }
                _ => {} // Skip non-watchable sources
            }
        }
        
        Ok(())
    }
    
    async fn stop_watching(&self) -> Result<()> {
        let mut watchers = self.watchers.write().await;
        for (_, watcher) in watchers.drain() {
            if let Some(handle) = watcher.handle {
                handle.abort();
            }
        }
        
        let mut task_handles = self.task_handles.write().await;
        for handle in task_handles.drain(..) {
            handle.abort();
        }
        
        Ok(())
    }
    
    async fn subscribe_to_changes(&self) -> Result<broadcast::Receiver<ConfigurationEvent>> {
        Ok(self.event_sender.subscribe())
    }
    
    async fn register_change_handler(&self, handler: Arc<dyn ConfigurationChangeHandler>) -> Result<()> {
        let mut handlers = self.change_handlers.write().await;
        handlers.push(handler);
        
        let mut stats = self.statistics.write().await;
        stats.change_handlers = handlers.len();
        
        Ok(())
    }
    
    async fn register_validator(&self, validator: Arc<dyn ConfigurationValidator>) -> Result<()> {
        let mut validators = self.validators.write().await;
        validators.push(validator);
        
        let mut stats = self.statistics.write().await;
        stats.validators = validators.len();
        
        Ok(())
    }
    
    async fn validate_configuration(&self) -> Result<ValidationResult> {
        let config = self.current_config.read().await;
        self.internal_validate(&config).await
    }
    
    async fn create_snapshot(&self, description: String) -> Result<ConfigurationSnapshot> {
        let config = {
            let current_config = self.current_config.read().await;
            current_config.clone()
        };
        
        let metadata = {
            let metadata = self.metadata.read().await;
            metadata.clone()
        };
        
        let snapshot = ConfigurationSnapshot {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            config,
            metadata,
            description,
        };
        
        let mut snapshots = self.snapshots.write().await;
        snapshots.push(snapshot.clone());
        
        // Limit snapshots
        if snapshots.len() > self.manager_config.max_snapshots {
            snapshots.remove(0);
        }
        
        let mut stats = self.statistics.write().await;
        stats.snapshots_count = snapshots.len();
        
        Ok(snapshot)
    }
    
    async fn restore_snapshot(&self, snapshot_id: Uuid) -> Result<()> {
        let snapshot = {
            let snapshots = self.snapshots.read().await;
            snapshots.iter()
                .find(|s| s.id == snapshot_id)
                .cloned()
                .ok_or_else(|| BedrockError::ConfigError("Snapshot not found".into()))?
        };
        
        let old_config = {
            let config = self.current_config.read().await;
            config.clone()
        };
        
        // Restore configuration
        {
            let mut current_config = self.current_config.write().await;
            *current_config = snapshot.config.clone();
        }
        {
            let mut metadata = self.metadata.write().await;
            *metadata = snapshot.metadata;
        }
        
        // Notify change handlers
        self.notify_change_handlers("restore", Some(&old_config), &snapshot.config).await?;
        
        // Emit event
        let _ = self.event_sender.send(ConfigurationEvent::ConfigurationReloaded {
            config_type: "snapshot_restore".to_string(),
            changes: vec![], // Could compute diff here
        });
        
        Ok(())
    }
    
    async fn get_snapshots(&self) -> Result<Vec<ConfigurationSnapshot>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.clone())
    }
    
    async fn get_statistics(&self) -> Result<ConfigurationStatistics> {
        let stats = self.statistics.read().await;
        Ok(stats.clone())
    }
}

// Placeholder implementations for missing dependencies
impl ConfigurationManagerImpl {
    async fn start_file_watcher(&self, _path: PathBuf) -> Result<()> {
        // Implementation would use notify crate or similar
        Ok(())
    }
    
    async fn start_directory_watcher(&self, _path: PathBuf) -> Result<()> {
        // Implementation would watch directory for changes
        Ok(())
    }
    
    async fn start_remote_watcher(&self, _source: ConfigurationSource, _interval: Duration) -> Result<()> {
        // Implementation would poll remote endpoint
        Ok(())
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // Simplified glob matching - would use glob crate in production
    if pattern == "*" {
        return true;
    }
    
    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return text.starts_with(parts[0]) && text.ends_with(parts[1]);
        }
    }
    
    pattern == text
}

impl Default for ConfigurationStatistics {
    fn default() -> Self {
        Self {
            total_sources: 0,
            active_watchers: 0,
            total_reloads: 0,
            successful_reloads: 0,
            failed_reloads: 0,
            last_reload: None,
            configuration_size: 0,
            snapshots_count: 0,
            validation_errors: 0,
            change_handlers: 0,
            validators: 0,
        }
    }
}

// Example change handler
#[derive(Debug)]
pub struct McpConfigurationHandler;

#[async_trait]
impl ConfigurationChangeHandler for McpConfigurationHandler {
    async fn handle_change(
        &self,
        change_type: &str,
        _old_config: Option<&serde_json::Value>,
        new_config: &serde_json::Value,
    ) -> Result<()> {
        tracing::info!("MCP configuration changed: {}", change_type);
        
        // Handle MCP-specific configuration changes
        if let Some(mcp_config) = new_config.get("mcp") {
            tracing::debug!("MCP configuration updated: {}", mcp_config);
            // Would trigger MCP service reconfiguration
        }
        
        Ok(())
    }
    
    fn handler_name(&self) -> &str {
        "McpConfigurationHandler"
    }
    
    fn interested_paths(&self) -> Vec<String> {
        vec!["mcp".to_string(), "mcp.servers".to_string()]
    }
}

// Example validator
#[derive(Debug)]
pub struct McpConfigurationValidator;

#[async_trait]
impl ConfigurationValidator for McpConfigurationValidator {
    async fn validate(&self, config: &serde_json::Value) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        if let Some(mcp_config) = config.get("mcp") {
            if let Some(servers) = mcp_config.get("servers") {
                if let serde_json::Value::Object(server_map) = servers {
                    for (name, server_config) in server_map {
                        // Validate server configuration
                        if server_config.get("command").is_none() && server_config.get("url").is_none() {
                            errors.push(ValidationError {
                                path: format!("mcp.servers.{}", name),
                                message: "Server must have either 'command' or 'url'".to_string(),
                                error_code: "MISSING_TRANSPORT".to_string(),
                            });
                        }
                    }
                } else {
                    errors.push(ValidationError {
                        path: "mcp.servers".to_string(),
                        message: "Servers configuration must be an object".to_string(),
                        error_code: "INVALID_TYPE".to_string(),
                    });
                }
            }
        }
        
        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }
    
    fn validator_name(&self) -> &str {
        "McpConfigurationValidator"
    }
    
    fn config_type(&self) -> &str {
        "mcp"
    }
}
use async_trait::async_trait;
use bedrock_client::BedrockClient;
use bedrock_config::AgentConfig;
use bedrock_core::{Agent as AgentTrait, BedrockError, Result, Task, TaskResult, TaskStatus};
use bedrock_mcp::{McpConfig, McpManager};
use bedrock_metrics::{CostCalculator, MetricsCollector, TokenTracker};
use bedrock_task::{Priority, TaskExecutor, TaskPersistence, TaskQueue};
use bedrock_tools::ToolRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

pub struct Agent {
    config: Arc<AgentConfig>,
    #[allow(dead_code)]
    bedrock_client: Arc<BedrockClient>,
    tool_registry: Arc<ToolRegistry>,
    task_queue: Arc<TaskQueue>,
    task_executor: Arc<TaskExecutor>,
    task_persistence: TaskPersistence,
    token_tracker: Arc<TokenTracker>,
    cost_calculator: Arc<CostCalculator>,
    metrics_collector: Arc<tokio::sync::RwLock<MetricsCollector>>,
    mcp_manager: Option<Arc<tokio::sync::Mutex<McpManager>>>,
}

impl Agent {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let bedrock_client = Arc::new(BedrockClient::new(config.clone()).await?);
        let tool_registry = Arc::new(ToolRegistry::new());
        let task_queue = Arc::new(TaskQueue::new(100));
        
        let task_executor = Arc::new(TaskExecutor::new(
            Arc::clone(&bedrock_client),
            Arc::clone(&tool_registry),
            Arc::new(config.clone()),
        ));
        
        let task_persistence = TaskPersistence::new(&config.paths.home_dir);
        let token_tracker = Arc::new(TokenTracker::new());
        let cost_calculator = Arc::new(CostCalculator::from_config(&config));
        let metrics_collector = Arc::new(tokio::sync::RwLock::new(MetricsCollector::new()));
        
        info!("Agent initialized with model: {}", config.agent.model);
        
        Ok(Self {
            config: Arc::new(config),
            bedrock_client,
            tool_registry,
            task_queue,
            task_executor,
            task_persistence,
            token_tracker,
            cost_calculator,
            metrics_collector,
            mcp_manager: None,
        })
    }

    pub async fn from_config_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let config = AgentConfig::from_yaml(path)?;
        Self::new(config).await
    }

    pub async fn from_default_config() -> Result<Self> {
        let config_path = AgentConfig::default_config_path();
        if config_path.exists() {
            Self::from_config_file(config_path).await
        } else {
            Err(BedrockError::ConfigError(
                "No configuration file found. Please create agent.yaml".into()
            ))
        }
    }

    pub async fn initialize_mcp(&mut self, mcp_config: McpConfig) -> Result<()> {
        let mcp_manager = McpManager::new(Arc::clone(&self.tool_registry));
        self.mcp_manager = Some(Arc::new(tokio::sync::Mutex::new(mcp_manager)));
        
        info!("MCP manager initialized with {} servers", mcp_config.servers.len());
        Ok(())
    }

    pub async fn enqueue_task(&self, task: Task, priority: Priority) -> Result<()> {
        self.task_queue.enqueue(task, priority).await
    }

    pub async fn process_queue(&self) -> Result<()> {
        while let Some(queued_task) = self.task_queue.dequeue().await {
            info!("Processing task: {}", queued_task.task.task_id);
            let result = self.execute_task(queued_task.task).await?;
            self.task_persistence.save(&result).await?;
        }
        Ok(())
    }

    pub fn get_tool_registry(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.tool_registry)
    }

    pub fn get_token_tracker(&self) -> Arc<TokenTracker> {
        Arc::clone(&self.token_tracker)
    }

    pub fn get_cost_calculator(&self) -> Arc<CostCalculator> {
        Arc::clone(&self.cost_calculator)
    }

    pub async fn get_metrics_summary(&self) -> bedrock_metrics::MetricsSummary {
        self.metrics_collector.read().await.get_summary()
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down agent");
        
        if let Some(mcp_manager) = &self.mcp_manager {
            let mut manager = mcp_manager.lock().await;
            manager.close_all().await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl AgentTrait for Agent {
    #[instrument(skip(self), fields(task_id = %task.task_id))]
    async fn execute_task(&self, task: Task) -> Result<TaskResult> {
        info!("Executing task: {}", task.task_id);
        let start_time = std::time::Instant::now();
        
        let result = self.task_executor.execute(task.clone()).await;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = matches!(result, Ok(ref r) if r.status == TaskStatus::Completed);
        
        {
            let mut metrics = self.metrics_collector.write().await;
            metrics.record_request(duration_ms, success);
        }
        
        if let Ok(ref task_result) = result {
            self.token_tracker.add_input(
                task_result.token_stats.input_tokens,
                &self.config.agent.model,
            );
            self.token_tracker.add_output(
                task_result.token_stats.output_tokens,
                &self.config.agent.model,
            );
            
            let cost = self.cost_calculator.calculate(
                &task_result.token_stats,
                &self.config.agent.model,
            );
            
            info!(
                "Task {} completed. Tokens: {}, Cost: ${:.4}",
                task.task_id, task_result.token_stats.total_tokens, cost.total_cost
            );
            
            self.task_persistence.save(task_result).await?;
        }
        
        result
    }

    async fn cancel_task(&self, _task_id: &Uuid) -> Result<()> {
        Err(BedrockError::TaskError("Task cancellation not yet implemented".into()))
    }

    async fn get_task_status(&self, task_id: &Uuid) -> Result<TaskStatus> {
        match self.task_persistence.load(task_id).await {
            Ok(result) => Ok(result.status),
            Err(_) => Ok(TaskStatus::Pending),
        }
    }
}

pub struct AgentBuilder {
    config: Option<AgentConfig>,
    config_path: Option<PathBuf>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            config_path: None,
        }
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn with_config_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    pub async fn build(self) -> Result<Agent> {
        if let Some(config) = self.config {
            Agent::new(config).await
        } else if let Some(path) = self.config_path {
            Agent::from_config_file(path).await
        } else {
            Agent::from_default_config().await
        }
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_builder() {
        let config = AgentConfig {
            agent: bedrock_config::AgentSettings {
                name: "test".to_string(),
                model: "claude-3".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
            },
            aws: bedrock_config::AwsSettings {
                region: "us-east-1".to_string(),
                profile: Some("default".to_string()),
                role_arn: None,
            },
            tools: bedrock_config::ToolSettings {
                allowed: vec![],
                permissions: Default::default(),
            },
            pricing: Default::default(),
            limits: Default::default(),
            paths: Default::default(),
        };

        let agent = AgentBuilder::new()
            .with_config(config)
            .build()
            .await;
        
        assert!(agent.is_ok());
    }
}
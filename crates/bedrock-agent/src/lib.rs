use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use bedrock_client::{BedrockClient, ToolDefinition};
use bedrock_config::AgentConfig;
use bedrock_core::{
    Agent as AgentTrait, BedrockError, CostDetails, Result, StreamResult,
    Task, TaskResult, TaskStatus, TokenStatistics,
};
use bedrock_mcp::McpManager;
use bedrock_task::TaskExecutor;
use bedrock_tools::ToolRegistry;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

pub struct Agent {
    config: Arc<AgentConfig>,
    bedrock_client: Arc<BedrockClient>,
    tool_registry: Arc<ToolRegistry>,
    task_executor: Arc<TaskExecutor>,
    mcp_manager: Option<Arc<tokio::sync::RwLock<McpManager>>>,
}

impl Agent {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let bedrock_client = Arc::new(BedrockClient::new(config.clone()).await?);
        
        // Initialize tool registry with default tools
        let tool_registry = Arc::new(
            ToolRegistry::with_default_tools(&config.paths.workspace_dir)
        );
        
        // Initialize MCP manager if enabled
        let mcp_manager = if config.mcp.enabled {
            info!("Initializing MCP integration");
            let mut manager = McpManager::new(tool_registry.clone());
            
            // Load MCP configurations
            for config_file in &config.mcp.config_files {
                if let Err(e) = manager.load_config_file(config_file).await {
                    warn!("Failed to load MCP config from {}: {}", config_file, e);
                }
            }
            
            // Add inline MCP servers from agent config
            if !config.mcp.inline_servers.is_empty() {
                let mut mcp_servers = std::collections::HashMap::new();
                for (name, value) in &config.mcp.inline_servers {
                    match serde_json::from_value::<bedrock_mcp::McpServerConfig>(value.clone()) {
                        Ok(server_config) => {
                            mcp_servers.insert(name.clone(), server_config);
                        }
                        Err(e) => {
                            warn!("Failed to parse inline MCP server '{}': {}", name, e);
                        }
                    }
                }
                if !mcp_servers.is_empty() {
                    if let Err(e) = manager.add_servers_from_config(mcp_servers).await {
                        warn!("Failed to add inline MCP servers: {}", e);
                    }
                }
            }
            
            // Start specified MCP servers
            if let Err(e) = manager.start_servers(config.mcp.servers.clone()).await {
                warn!("Failed to start MCP servers: {}", e);
                // Continue even if MCP servers fail to start
            }
            
            Some(Arc::new(tokio::sync::RwLock::new(manager)))
        } else {
            None
        };
        
        let task_executor = Arc::new(TaskExecutor::new(
            Arc::clone(&bedrock_client),
            Arc::clone(&tool_registry),
            Arc::new(config.clone()),
        ));
        
        Ok(Self {
            config: Arc::new(config),
            bedrock_client,
            tool_registry,
            task_executor,
            mcp_manager,
        })
    }

    pub async fn from_config_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let config = AgentConfig::from_yaml(path)?;
        Self::new(config).await
    }

    pub fn get_tool_registry(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.tool_registry)
    }

    pub fn get_client(&self) -> Arc<BedrockClient> {
        Arc::clone(&self.bedrock_client)
    }

    #[instrument(skip(self, prompt))]
    pub async fn chat(&self, prompt: &str) -> Result<String> {
        info!("Processing chat prompt");
        
        // Build tool definitions if tools are available
        let tool_definitions = if !self.tool_registry.list().is_empty() {
            Some(
                self.tool_registry
                    .get_all()
                    .into_iter()
                    .map(|tool| ToolDefinition {
                        name: tool.name().to_string(),
                        description: tool.description().to_string(),
                        input_schema: tool.schema(),
                    })
                    .collect()
            )
        } else {
            None
        };

        // Create user message
        let user_message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))?;

        let mut conversation = vec![user_message];
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                warn!("Maximum iterations reached");
                return Ok("I apologize, but I couldn't complete the task within the allowed iterations.".to_string());
            }

            // Call the model
            let response = self.bedrock_client
                .converse(
                    &self.config.agent.model,
                    conversation.clone(),
                    Some(self.config.agent.get_system_prompt()),
                    tool_definitions.clone(),
                )
                .await?;

            // Add assistant response to conversation
            conversation.push(response.message.clone());

            // Check if we need to handle tool calls
            if response.has_tool_use() {
                let tool_uses = response.get_tool_uses();
                
                if !tool_uses.is_empty() {
                    debug!("Processing {} tool calls", tool_uses.len());
                    
                    // Execute tools
                    let tool_results = self.bedrock_client
                        .execute_tools(&tool_uses, &self.tool_registry)
                        .await?;
                    
                    // Create tool result message
                    let tool_result_message = Message::builder()
                        .role(ConversationRole::User)
                        .set_content(Some(
                            tool_results
                                .into_iter()
                                .map(ContentBlock::ToolResult)
                                .collect(),
                        ))
                        .build()
                        .map_err(|e| BedrockError::Unknown(e.to_string()))?;
                    
                    conversation.push(tool_result_message);
                    
                    // Continue conversation
                    continue;
                }
            }

            // No more tool calls, return the response
            return Ok(response.get_text_content());
        }
    }

    fn calculate_cost(&self, input_tokens: usize, output_tokens: usize) -> CostDetails {
        let pricing = self.config.pricing.get(&self.config.agent.model);
        
        let (input_cost, output_cost, currency) = if let Some(pricing) = pricing {
            let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_per_1k;
            let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_per_1k;
            (input_cost, output_cost, pricing.currency.clone())
        } else {
            // Default pricing if model not in config
            let input_cost = (input_tokens as f64 / 1000.0) * 0.003;
            let output_cost = (output_tokens as f64 / 1000.0) * 0.015;
            (input_cost, output_cost, "USD".to_string())
        };
        
        CostDetails {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            currency,
            model: self.config.agent.model.clone(),
        }
    }

    pub async fn chat_stream(
        &self,
        prompt: &str,
        mut callback: impl FnMut(&str) + Send,
    ) -> Result<StreamResult> {
        info!("Processing streaming chat prompt");
        
        // Build tool definitions if tools are available
        let tool_definitions = if !self.tool_registry.list().is_empty() {
            Some(
                self.tool_registry
                    .get_all()
                    .into_iter()
                    .map(|tool| ToolDefinition {
                        name: tool.name().to_string(),
                        description: tool.description().to_string(),
                        input_schema: tool.schema(),
                    })
                    .collect()
            )
        } else {
            None
        };

        // Create user message
        let user_message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))?;

        let mut conversation = vec![user_message];
        let mut iterations = 0;
        let mut total_input_tokens = 0usize;
        let mut total_output_tokens = 0usize;
        let final_response;
        const MAX_ITERATIONS: usize = 10;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                warn!("Maximum iterations reached");
                let msg = "I apologize, but I couldn't complete the task within the allowed iterations.";
                callback(msg);
                final_response = msg.to_string();
                break;
            }

            // Get streaming response - this now returns a ConverseResponse with the full message
            let response = self.bedrock_client
                .converse_stream(
                    &self.config.agent.model,
                    conversation.clone(),
                    Some(self.config.agent.get_system_prompt()),
                    tool_definitions.clone(),
                )
                .await?;

            // Track token usage
            if let Some(usage) = &response.usage {
                total_input_tokens += usage.input_tokens() as usize;
                total_output_tokens += usage.output_tokens() as usize;
            }

            // Add assistant response to conversation
            conversation.push(response.message.clone());

            // Check if we need to handle tool calls
            if response.has_tool_use() {
                let tool_uses = response.get_tool_uses();
                
                if !tool_uses.is_empty() {
                    debug!("Processing {} tool calls", tool_uses.len());
                    
                    // Execute tools
                    let tool_results = self.bedrock_client
                        .execute_tools(&tool_uses, &self.tool_registry)
                        .await?;
                    
                    // Create tool result message
                    let tool_result_message = Message::builder()
                        .role(ConversationRole::User)
                        .set_content(Some(
                            tool_results
                                .into_iter()
                                .map(ContentBlock::ToolResult)
                                .collect(),
                        ))
                        .build()
                        .map_err(|e| BedrockError::Unknown(e.to_string()))?;
                    
                    conversation.push(tool_result_message);
                    
                    // Continue conversation
                    continue;
                }
            }

            // No more tool calls, capture the response
            final_response = response.get_text_content();
            break;
        }

        // Calculate cost and prepare result
        let token_stats = TokenStatistics {
            input_tokens: total_input_tokens,
            output_tokens: total_output_tokens,
            total_tokens: total_input_tokens + total_output_tokens,
            cache_hits: 0,
        };

        let cost = self.calculate_cost(total_input_tokens, total_output_tokens);

        Ok(StreamResult {
            response: final_response,
            token_stats,
            cost,
        })
    }
    
    /// Shutdown the agent and cleanup resources
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down agent");
        
        // Stop all MCP servers if initialized
        if let Some(mcp_manager) = &self.mcp_manager {
            let mut manager = mcp_manager.write().await;
            if let Err(e) = manager.stop_all().await {
                warn!("Error stopping MCP servers: {}", e);
            }
        }
        
        Ok(())
    }
    
    /// Get list of connected MCP servers
    pub async fn list_mcp_servers(&self) -> Vec<String> {
        if let Some(mcp_manager) = &self.mcp_manager {
            let manager = mcp_manager.read().await;
            manager.list_servers().await
        } else {
            vec![]
        }
    }
}

#[async_trait]
impl AgentTrait for Agent {
    async fn execute_task(&self, task: Task) -> Result<TaskResult> {
        info!("Executing task: {}", task.task_id);
        
        // Execute the task
        let result = self.task_executor.execute_task(task).await?;
        
        // Save the result
        self.task_executor.save_result(&result).await?;
        
        Ok(result)
    }

    async fn cancel_task(&self, task_id: &Uuid) -> Result<()> {
        info!("Cancelling task: {}", task_id);
        // Task cancellation would be implemented here
        Ok(())
    }

    async fn get_task_status(&self, task_id: &Uuid) -> Result<TaskStatus> {
        info!("Getting task status: {}", task_id);
        
        // Try to load the result
        match self.task_executor.load_result(task_id).await {
            Ok(result) => Ok(result.status),
            Err(_) => Ok(TaskStatus::Pending),
        }
    }
}
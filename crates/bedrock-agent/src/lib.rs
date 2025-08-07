use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use bedrock_client::{BedrockClient, ToolDefinition};
use bedrock_config::AgentConfig;
use bedrock_core::{Agent as AgentTrait, BedrockError, Result, Task, TaskResult, TaskStatus};
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
}

impl Agent {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let bedrock_client = Arc::new(BedrockClient::new(config.clone()).await?);
        
        // Initialize tool registry with default tools
        let tool_registry = Arc::new(
            ToolRegistry::with_default_tools(&config.paths.workspace_dir)
        );
        
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

    pub async fn chat_stream(
        &self,
        prompt: &str,
        mut callback: impl FnMut(&str) + Send,
    ) -> Result<String> {
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
        const MAX_ITERATIONS: usize = 10;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                warn!("Maximum iterations reached");
                let msg = "I apologize, but I couldn't complete the task within the allowed iterations.";
                callback(msg);
                return Ok(msg.to_string());
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
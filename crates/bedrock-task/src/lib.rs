use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message,
};
use bedrock_client::{BedrockClient, ToolDefinition};
use bedrock_config::AgentConfig;
use bedrock_conversation::{ConversationManager, TokenUsageStats};
use bedrock_core::{
    BedrockError, CostDetails, Result, Task, TaskResult, TaskStatus,
    TokenStatistics,
};
use bedrock_tools::ToolRegistry;
use chrono::Utc;
use serde_json::Value;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Priority {
    High = 3,
    Normal = 2,
    Low = 1,
}

#[derive(Debug, Clone)]
pub struct QueuedTask {
    pub task: Task,
    pub priority: Priority,
    pub queued_at: chrono::DateTime<chrono::Utc>,
}

impl PartialEq for QueuedTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.task_id == other.task.task_id
    }
}

impl Eq for QueuedTask {}

impl PartialOrd for QueuedTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.priority.clone() as u8)
            .cmp(&(other.priority.clone() as u8))
            .then_with(|| other.queued_at.cmp(&self.queued_at))
    }
}

pub struct TaskExecutor {
    bedrock_client: Arc<BedrockClient>,
    tool_registry: Arc<ToolRegistry>,
    config: Arc<AgentConfig>,
    task_queue: Arc<Mutex<BinaryHeap<QueuedTask>>>,
    active_tasks: Arc<Mutex<Vec<Uuid>>>,
    max_concurrent_tasks: usize,
    max_tool_iterations: usize,
    conversation_manager: Arc<Mutex<ConversationManager>>,
}

impl TaskExecutor {
    pub fn new(
        bedrock_client: Arc<BedrockClient>,
        tool_registry: Arc<ToolRegistry>,
        config: Arc<AgentConfig>,
    ) -> Result<Self> {
        let conversation_manager = ConversationManager::new()?;
        Ok(Self {
            bedrock_client,
            tool_registry,
            config,
            task_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            active_tasks: Arc::new(Mutex::new(Vec::new())),
            max_concurrent_tasks: 3,
            max_tool_iterations: 10,
            conversation_manager: Arc::new(Mutex::new(conversation_manager)),
        })
    }

    pub async fn queue_task(&self, task: Task, priority: Priority) -> Result<()> {
        let mut queue = self.task_queue.lock().await;
        queue.push(QueuedTask {
            task,
            priority,
            queued_at: Utc::now(),
        });
        info!("Task queued. Queue size: {}", queue.len());
        Ok(())
    }

    pub async fn process_queue(&self) {
        loop {
            let active_count = self.active_tasks.lock().await.len();
            if active_count >= self.max_concurrent_tasks {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let task = {
                let mut queue = self.task_queue.lock().await;
                queue.pop()
            };

            if let Some(queued_task) = task {
                let executor = self.clone();
                tokio::spawn(async move {
                    let task_id = queued_task.task.task_id;
                    {
                        let mut active = executor.active_tasks.lock().await;
                        active.push(task_id);
                    }

                    let _result = executor.execute_task(queued_task.task).await;

                    {
                        let mut active = executor.active_tasks.lock().await;
                        active.retain(|&id| id != task_id);
                    }
                });
            } else {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    #[instrument(skip(self, task), fields(task_id = %task.task_id))]
    pub async fn execute_task(&self, task: Task) -> Result<TaskResult> {
        info!("Executing task: {}", task.task_id);

        if task.prompt.is_empty() {
            return Err(BedrockError::TaskError("Task prompt is empty".into()));
        }

        let task_timeout = Duration::from_secs(300); // 5 minute default timeout
        
        match timeout(task_timeout, self.execute_internal(task.clone())).await {
            Ok(result) => result,
            Err(_) => {
                error!("Task {} timed out after 300 seconds", task.task_id);
                Ok(TaskResult {
                    task_id: task.task_id,
                    status: TaskStatus::Failed,
                    summary: "Task timed out".to_string(),
                    conversation: Some(vec![]),
                    result: None,
                    token_stats: TokenStatistics::default(),
                    cost: CostDetails::default(),
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    duration_ms: Some(300_000),
                    error: Some("Task timed out after 300 seconds".to_string()),
                })
            }
        }
    }

    async fn execute_internal(&self, task: Task) -> Result<TaskResult> {
        let started_at = Utc::now();
        
        if !self.tool_registry.list().is_empty() {
            self.execute_with_tools(task, started_at).await
        } else {
            self.execute_without_tools(task, started_at).await
        }
    }

    #[instrument(skip(self, task), fields(task_id = %task.task_id))]
    async fn execute_with_tools(
        &self,
        task: Task,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<TaskResult> {
        info!("Starting task execution with {} tools", self.tool_registry.list().len());

        // Build tool definitions
        let all_tools = self.tool_registry.get_all();
        debug!("Building tool definitions for {} tools", all_tools.len());
        
        // Limit tools to max_tools setting from config (default 64, Bedrock limit)
        let max_tools = self.config.mcp.max_tools;
        let tools_to_use = if all_tools.len() > max_tools {
            warn!(
                "Tool count ({}) exceeds max_tools limit ({}). Limiting to first {} tools.",
                all_tools.len(), max_tools, max_tools
            );
            all_tools.into_iter().take(max_tools).collect()
        } else {
            all_tools
        };
        
        let tool_definitions: Vec<ToolDefinition> = tools_to_use
            .into_iter()
            .map(|tool| {
                debug!("Processing tool: {}", tool.name());
                let schema = tool.schema();
                debug!("Got schema for tool: {}, size: {} bytes", 
                    tool.name(), 
                    serde_json::to_string(&schema).unwrap_or_default().len()
                );
                ToolDefinition {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    input_schema: schema,
                }
            })
            .collect();
        
        debug!("Built {} tool definitions (limited from {} total)", 
            tool_definitions.len(), 
            self.tool_registry.list().len()
        );

        // Initialize conversation with user prompt
        let user_message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(task.prompt.clone()))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))?;

        let mut conversation = vec![user_message];
        let mut total_tokens = TokenStatistics::default();

        // Execute conversation with tool support
        let mut iterations = 0;
        loop {
            iterations += 1;
            if iterations > self.max_tool_iterations {
                warn!("Maximum tool iterations reached");
                break;
            }

            // Call the model
            let response = self.bedrock_client
                .converse(
                    &self.config.agent.model,
                    conversation.clone(),
                    if task.context.is_empty() {
                        None
                    } else {
                        Some(task.context.clone())
                    },
                    if tool_definitions.is_empty() {
                        None
                    } else {
                        Some(tool_definitions.clone())
                    },
                )
                .await?;

            // Update token statistics
            if let Some(usage) = &response.usage {
                total_tokens.input_tokens += usage.input_tokens() as usize;
                total_tokens.output_tokens += usage.output_tokens() as usize;
                total_tokens.total_tokens += usage.total_tokens() as usize;
            }

            // Add assistant response to conversation
            conversation.push(response.message.clone());

            // Check if we need to handle tool calls
            debug!("Response stop_reason: {:?}, has_tool_use: {}", 
                response.stop_reason, response.has_tool_use());
            
            if response.has_tool_use() {
                // Get tool uses from the response
                let tool_uses = response.get_tool_uses();
                
                if !tool_uses.is_empty() {
                    debug!("Processing {} tool calls", tool_uses.len());
                    
                    // Execute tools and get results
                    let tool_results = self.bedrock_client
                        .execute_tools(&tool_uses, &self.tool_registry)
                        .await?;
                    
                    // Create a message with tool results
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
                    
                    // Continue conversation with tool results
                    continue;
                }
            }

            // No more tool calls, task is complete
            let cost = self.calculate_cost(&total_tokens);
            let text_content = response.get_text_content();
            let summary = if text_content.is_empty() {
                "Task completed".to_string()
            } else {
                self.generate_summary(&text_content)
            };

            // Convert conversation to JSON for storage
            let conversation_json = self.messages_to_json(&conversation)?;

            let duration_ms = (Utc::now() - started_at).num_milliseconds() as u64;
            return Ok(TaskResult {
                task_id: task.task_id,
                status: TaskStatus::Completed,
                summary: summary.clone(),
                conversation: Some(conversation_json),
                result: Some(serde_json::json!({"summary": summary})),
                token_stats: total_tokens,
                cost,
                started_at,
                completed_at: Some(Utc::now()),
                duration_ms: Some(duration_ms),
                error: None,
            });
        }

        // Max iterations reached
        let cost = self.calculate_cost(&total_tokens);
        let conversation_json = self.messages_to_json(&conversation)?;
        
        let duration_ms = (Utc::now() - started_at).num_milliseconds() as u64;
        Ok(TaskResult {
            task_id: task.task_id,
            status: TaskStatus::Failed,
            summary: "Task failed: max tool iterations reached".to_string(),
            conversation: Some(conversation_json),
            result: None,
            token_stats: total_tokens,
            cost,
            started_at,
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration_ms),
            error: Some("Max tool iterations reached".to_string()),
        })
    }

    async fn execute_without_tools(
        &self,
        task: Task,
        started_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<TaskResult> {
        info!("Executing task without tools");

        // Initialize conversation with user prompt
        let user_message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(task.prompt.clone()))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))?;

        let conversation = vec![user_message];

        // Call the model
        let response = self.bedrock_client
            .converse(
                &self.config.agent.model,
                conversation.clone(),
                if task.context.is_empty() {
                    None
                } else {
                    Some(task.context.clone())
                },
                None,
            )
            .await?;

        // Calculate token statistics
        let mut total_tokens = TokenStatistics::default();
        if let Some(usage) = &response.usage {
            total_tokens.input_tokens = usage.input_tokens() as usize;
            total_tokens.output_tokens = usage.output_tokens() as usize;
            total_tokens.total_tokens = usage.total_tokens() as usize;
        }

        let cost = self.calculate_cost(&total_tokens);
        let text_content = response.get_text_content();
        let summary = if text_content.is_empty() {
            "Task completed".to_string()
        } else {
            self.generate_summary(&text_content)
        };

        // Build final conversation with response
        let mut final_conversation = conversation;
        final_conversation.push(response.message);
        
        let conversation_json = self.messages_to_json(&final_conversation)?;

        let duration_ms = (Utc::now() - started_at).num_milliseconds() as u64;
        Ok(TaskResult {
            task_id: task.task_id,
            status: TaskStatus::Completed,
            summary: summary.clone(),
            conversation: Some(conversation_json),
            result: Some(serde_json::json!({"summary": summary})),
            token_stats: total_tokens,
            cost,
            started_at,
            completed_at: Some(Utc::now()),
            duration_ms: Some(duration_ms),
            error: None,
        })
    }

    fn calculate_cost(&self, tokens: &TokenStatistics) -> CostDetails {
        // Get pricing for the model being used
        let pricing = self.config.pricing.get(&self.config.agent.model);
        
        let (input_cost, output_cost, currency) = if let Some(pricing) = pricing {
            let input_cost = (tokens.input_tokens as f64 / 1000.0) * pricing.input_per_1k;
            let output_cost = (tokens.output_tokens as f64 / 1000.0) * pricing.output_per_1k;
            (input_cost, output_cost, pricing.currency.clone())
        } else {
            // Default pricing if model not in config
            let input_cost = (tokens.input_tokens as f64 / 1000.0) * 0.003;
            let output_cost = (tokens.output_tokens as f64 / 1000.0) * 0.015;
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

    fn generate_summary(&self, content: &str) -> String {
        if content.len() <= 100 {
            content.to_string()
        } else {
            let summary = content.chars().take(97).collect::<String>();
            format!("{summary}...")
        }
    }

    // Convert AWS SDK Messages to JSON for storage
    fn messages_to_json(&self, messages: &[Message]) -> Result<Vec<Value>> {
        let mut json_messages = Vec::new();
        
        for msg in messages {
            let role = format!("{:?}", msg.role());
            let content = msg.content()
                .iter()
                .filter_map(|block| {
                    if let Ok(text) = block.as_text() {
                        Some(text.to_string())
                    } else if let Ok(tool_use) = block.as_tool_use() {
                        Some(format!("[Tool: {}]", tool_use.name()))
                    } else if let Ok(_tool_result) = block.as_tool_result() {
                        Some("[Tool Result]".to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            json_messages.push(serde_json::json!({
                "role": role,
                "content": content,
                "timestamp": Utc::now().to_rfc3339()
            }));
        }
        
        Ok(json_messages)
    }

    pub async fn save_result(&self, result: &TaskResult) -> Result<()> {
        let mut conv_manager = self.conversation_manager.lock().await;
        
        // Start a new conversation if needed
        let conversation_id = if let Some(id) = conv_manager.current_conversation_id() {
            id
        } else {
            conv_manager.start_conversation(
                self.config.agent.model.clone(),
                Some(self.config.agent.get_system_prompt()),
            )?
        };
        
        // Save task results to conversation storage
        let tasks = serde_json::json!({
            "task_id": result.task_id,
            "status": result.status,
            "result": result.result,
            "error": result.error,
            "token_stats": result.token_stats,
            "cost": result.cost,
            "duration_ms": result.duration_ms,
        });
        
        conv_manager.save_task_results(tasks)?;
        
        // Also save conversation messages if available
        if let Some(conversation) = &result.conversation {
            for msg_json in conversation {
                // Convert JSON back to message entry format
                if let Some(role) = msg_json.get("role").and_then(|r| r.as_str()) {
                    let content = msg_json.get("content").unwrap_or(&Value::Null).clone();
                    
                    match role {
                        "user" => {
                            if let Some(text) = content.as_str() {
                                conv_manager.add_user_message(text.to_string())?;
                            }
                        },
                        "assistant" => {
                            if let Some(text) = content.as_str() {
                                let tokens = TokenUsageStats {
                                    input_tokens: result.token_stats.input_tokens as u32,
                                    output_tokens: result.token_stats.output_tokens as u32,
                                    total_tokens: result.token_stats.total_tokens as u32,
                                    total_cost: Some(result.cost.total_cost),
                                };
                                conv_manager.add_assistant_message(text.to_string(), Some(tokens))?;
                            }
                        },
                        _ => {}
                    }
                }
            }
        }
        
        // Also save to workspace/results for backward compatibility
        let results_dir = self.config.paths.workspace_dir.join("results");
        if !results_dir.exists() {
            std::fs::create_dir_all(&results_dir)
                .map_err(BedrockError::IoError)?;
        }

        let file_path = results_dir.join(format!("{}.json", result.task_id));
        let json = serde_json::to_string_pretty(result)?;
        std::fs::write(file_path, json)
            .map_err(BedrockError::IoError)?;
        
        info!("Task result saved to conversation: {} (task: {})", 
              conversation_id, result.task_id);
        Ok(())
    }

    pub async fn load_result(&self, task_id: &Uuid) -> Result<TaskResult> {
        // For now, maintain backward compatibility with workspace/results
        let results_dir = self.config.paths.workspace_dir.join("results");
        let file_path = results_dir.join(format!("{task_id}.json"));
        
        if file_path.exists() {
            let json = std::fs::read_to_string(file_path)
                .map_err(BedrockError::IoError)?;
            
            let result: TaskResult = serde_json::from_str(&json)?;
            Ok(result)
        } else {
            Err(BedrockError::NotFound(format!("Task result not found: {}", task_id)))
        }
    }
    
    /// Resume a conversation by ID
    pub async fn resume_conversation(&self, conversation_id: Uuid) -> Result<()> {
        let mut conv_manager = self.conversation_manager.lock().await;
        let messages = conv_manager.resume_conversation(conversation_id)?;
        
        info!("Resumed conversation {} with {} messages", 
              conversation_id, messages.len());
        Ok(())
    }
    
    /// List all conversations for the current workspace
    pub async fn list_conversations(&self) -> Result<Vec<bedrock_conversation::metadata::ConversationSummary>> {
        let conv_manager = self.conversation_manager.lock().await;
        conv_manager.list_conversations()
    }
}

impl Clone for TaskExecutor {
    fn clone(&self) -> Self {
        Self {
            bedrock_client: Arc::clone(&self.bedrock_client),
            tool_registry: Arc::clone(&self.tool_registry),
            config: Arc::clone(&self.config),
            task_queue: Arc::clone(&self.task_queue),
            active_tasks: Arc::clone(&self.active_tasks),
            max_concurrent_tasks: self.max_concurrent_tasks,
            max_tool_iterations: self.max_tool_iterations,
            conversation_manager: Arc::clone(&self.conversation_manager),
        }
    }
}
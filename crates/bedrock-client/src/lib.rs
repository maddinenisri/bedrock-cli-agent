use aws_config::Region;
use aws_sdk_bedrockruntime as bedrock;
use aws_sdk_bedrockruntime::types::{
    Message, StopReason, SystemContentBlock,
    Tool, ToolConfiguration, ToolResultBlock, ToolSpecification, ToolUseBlock,
    ToolInputSchema, ToolResultContentBlock,
};
use aws_smithy_types::Document;
use bedrock_config::{AgentConfig, AwsSettings};
use bedrock_core::{BedrockError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

pub mod ui;
mod streaming;
pub use ui::{display_tool_execution, display_tool_result, get_tool_display_name, get_tool_emoji};
use streaming::process_stream_with_response;

pub struct BedrockClient {
    client: bedrock::Client,
    region: Region,
    config: Arc<AgentConfig>,
}

// For non-streaming responses
#[derive(Debug)]
pub struct ConverseResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: Option<bedrock::types::TokenUsage>,
}

impl ConverseResponse {
    pub fn has_tool_use(&self) -> bool {
        matches!(self.stop_reason, StopReason::ToolUse)
    }

    pub fn get_tool_uses(&self) -> Vec<&ToolUseBlock> {
        self.message
            .content()
            .iter()
            .filter_map(|block| block.as_tool_use().ok())
            .collect()
    }

    pub fn get_text_content(&self) -> String {
        self.message
            .content()
            .iter()
            .filter_map(|block| {
                if let Ok(text) = block.as_text() {
                    Some(text.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}


impl BedrockClient {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let aws_config = Self::build_aws_config(&config.aws).await?;
        let client = bedrock::Client::new(&aws_config);
        let region = aws_config.region().cloned()
            .unwrap_or_else(|| Region::new(config.aws.region.clone()));

        Ok(Self {
            client,
            region,
            config: Arc::new(config),
        })
    }

    pub async fn from_config_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let config = AgentConfig::from_yaml(path)?;
        Self::new(config).await
    }

    async fn build_aws_config(settings: &AwsSettings) -> Result<aws_config::SdkConfig> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(Region::new(settings.region.clone()));

        if let Some(profile) = &settings.profile {
            info!("Using AWS profile: {}", profile);
            config_loader = config_loader.profile_name(profile);
        }

        let aws_config = config_loader.load().await;
        
        debug!("AWS config loaded for region: {}", settings.region);
        Ok(aws_config)
    }

    pub async fn converse(
        &self,
        model_id: &str,
        messages: Vec<Message>,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ConverseResponse> {
        let mut converse_request = self.client
            .converse()
            .model_id(model_id)
            .set_messages(Some(messages));

        if let Some(system_prompt) = system_prompt {
            let system_content = SystemContentBlock::Text(system_prompt);
            converse_request = converse_request.system(system_content);
        }

        let inference_config = bedrock::types::InferenceConfiguration::builder()
            .max_tokens(self.config.agent.max_tokens as i32)
            .temperature(self.config.agent.temperature)
            .build();

        converse_request = converse_request.inference_config(inference_config);

        if let Some(tools) = tools {
            let tool_config = self.build_tool_config(tools)?;
            converse_request = converse_request.tool_config(tool_config);
        }

        let response = converse_request.send().await
            .map_err(|e| BedrockError::Unknown(format!("Bedrock API error: {e}")))?;

        let message = response.output()
            .and_then(|output| output.as_message().ok())
            .ok_or_else(|| BedrockError::Unknown("No message in response".into()))?
            .clone();

        let stop_reason = response.stop_reason().clone();
        let usage = response.usage().cloned();

        Ok(ConverseResponse {
            message,
            stop_reason,
            usage,
        })
    }

    pub async fn converse_stream(
        &self,
        model_id: &str,
        messages: Vec<Message>,
        system_prompt: Option<String>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<ConverseResponse> {
        let mut converse_request = self.client
            .converse_stream()
            .model_id(model_id)
            .set_messages(Some(messages));

        if let Some(system_prompt) = system_prompt {
            let system_content = SystemContentBlock::Text(system_prompt);
            converse_request = converse_request.system(system_content);
        }

        let inference_config = bedrock::types::InferenceConfiguration::builder()
            .max_tokens(self.config.agent.max_tokens as i32)
            .temperature(self.config.agent.temperature)
            .build();

        converse_request = converse_request.inference_config(inference_config);

        if let Some(tools) = tools {
            let tool_config = self.build_tool_config(tools)?;
            converse_request = converse_request.tool_config(tool_config);
        }

        let stream_output = converse_request.send().await
            .map_err(|e| BedrockError::Unknown(format!("Bedrock streaming error: {e}")))?;

        // Create a stream that yields ConverseStreamOutput
        let stream = async_stream::stream! {
            let mut event_stream = stream_output.stream;
            loop {
                match event_stream.recv().await {
                    Ok(Some(output)) => {
                        yield Ok(output);
                    }
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(e);
                        break;
                    }
                }
            }
        };

        // Process the stream and reconstruct the full response
        process_stream_with_response(stream).await
    }

    fn build_tool_config(&self, tools: Vec<ToolDefinition>) -> Result<ToolConfiguration> {
        let mut tool_specs = Vec::new();
        
        info!("🔧 Building tool config for {} tools", tools.len());
        
        for tool in tools {
            debug!("Adding tool to Bedrock: {}", tool.name);
            
            // Following reference project pattern: fallback to empty schema on conversion failure
            let doc = match Self::json_to_document(&tool.input_schema) {
                Ok(d) => {
                    debug!("Schema converted successfully for tool: {}", tool.name);
                    d
                }
                Err(e) => {
                    warn!("Failed to convert schema for tool '{}': {} - using empty schema as fallback", tool.name, e);
                    // Use empty schema as fallback (reference project pattern)
                    Document::Object(std::collections::HashMap::new())
                }
            };
            
            let spec = ToolSpecification::builder()
                .name(tool.name.clone())
                .description(tool.description)
                .input_schema(ToolInputSchema::Json(doc))
                .build()
                .map_err(|e| BedrockError::Unknown(format!("Failed to build tool spec for '{}': {}", tool.name, e)))?;
            
            tool_specs.push(Tool::ToolSpec(spec));
        }
        
        info!("✅ Successfully built {} tool specifications", tool_specs.len());
        
        ToolConfiguration::builder()
            .set_tools(Some(tool_specs))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))
    }
    
    pub fn json_to_document(value: &Value) -> Result<Document> {
        Self::json_to_document_with_depth(value, 0)
    }
    
    fn json_to_document_with_depth(value: &Value, depth: usize) -> Result<Document> {
        const MAX_DEPTH: usize = 100; // Reasonable depth limit
        
        if depth > MAX_DEPTH {
            // Return a placeholder for deeply nested structures
            debug!("Max depth {} exceeded in json_to_document", MAX_DEPTH);
            return Ok(Document::String(format!("[Deep nested object at depth {}]", depth)));
        }
        
        match value {
            Value::Null => Ok(Document::Null),
            Value::Bool(b) => Ok(Document::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Document::Number(aws_smithy_types::Number::NegInt(i)))
                } else if let Some(u) = n.as_u64() {
                    Ok(Document::Number(aws_smithy_types::Number::PosInt(u)))
                } else if let Some(f) = n.as_f64() {
                    Ok(Document::Number(aws_smithy_types::Number::Float(f)))
                } else {
                    Err(BedrockError::Unknown("Invalid number".into()))
                }
            }
            Value::String(s) => Ok(Document::String(s.clone())),
            Value::Array(arr) => {
                let docs: Result<Vec<Document>> = arr.iter()
                    .map(|v| Self::json_to_document_with_depth(v, depth + 1))
                    .collect();
                Ok(Document::Array(docs?))
            }
            Value::Object(obj) => {
                let mut map = std::collections::HashMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::json_to_document_with_depth(v, depth + 1)?);
                }
                Ok(Document::Object(map))
            }
        }
    }

    fn document_to_json(doc: &Document) -> Result<Value> {
        match doc {
            Document::Null => Ok(Value::Null),
            Document::Bool(b) => Ok(Value::Bool(*b)),
            Document::Number(n) => {
                match n {
                    aws_smithy_types::Number::PosInt(u) => Ok(Value::Number((*u).into())),
                    aws_smithy_types::Number::NegInt(i) => Ok(Value::Number((*i).into())),
                    aws_smithy_types::Number::Float(f) => {
                        serde_json::Number::from_f64(*f)
                            .map(Value::Number)
                            .ok_or_else(|| BedrockError::Unknown("Invalid float value".into()))
                    }
                }
            }
            Document::String(s) => Ok(Value::String(s.clone())),
            Document::Array(arr) => {
                let values: Result<Vec<Value>> = arr.iter()
                    .map(Self::document_to_json)
                    .collect();
                Ok(Value::Array(values?))
            }
            Document::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::document_to_json(v)?);
                }
                Ok(Value::Object(map))
            }
        }
    }


    pub async fn execute_tools(
        &self,
        tool_uses: &[&ToolUseBlock],
        tool_registry: &bedrock_tools::ToolRegistry,
    ) -> Result<Vec<ToolResultBlock>> {
        let mut results = Vec::new();

        for tool_use in tool_uses {
            debug!("Executing tool: {}", tool_use.name());
            
            let result = if let Some(tool) = tool_registry.get(tool_use.name()) {
                let input_json = Self::document_to_json(tool_use.input())?;
                match tool.execute(input_json).await {
                    Ok(output) => {
                        let result_doc = Self::json_to_document(&output)?;
                        ToolResultBlock::builder()
                            .tool_use_id(tool_use.tool_use_id())
                            .content(ToolResultContentBlock::Json(result_doc))
                            .build()
                            .map_err(|e| BedrockError::Unknown(format!("Failed to build tool result: {e}")))?
                    }
                    Err(e) => {
                        let error_result = json!({
                            "error": e.to_string(),
                            "tool": tool_use.name()
                        });
                        let error_doc = Self::json_to_document(&error_result)?;
                        ToolResultBlock::builder()
                            .tool_use_id(tool_use.tool_use_id())
                            .content(ToolResultContentBlock::Json(error_doc))
                            .status(bedrock::types::ToolResultStatus::Error)
                            .build()
                            .map_err(|e| BedrockError::Unknown(format!("Failed to build error tool result: {e}")))?
                    }
                }
            } else {
                let error_result = json!({
                    "error": format!("Tool '{}' not found", tool_use.name()),
                    "tool": tool_use.name()
                });
                let error_doc = Self::json_to_document(&error_result)?;
                ToolResultBlock::builder()
                    .tool_use_id(tool_use.tool_use_id())
                    .content(ToolResultContentBlock::Json(error_doc))
                    .status(bedrock::types::ToolResultStatus::Error)
                    .build()
                    .map_err(|e| BedrockError::Unknown(format!("Failed to build error tool result: {e}")))?
            };
            
            results.push(result);
        }

        Ok(results)
    }

    pub fn get_region(&self) -> &str {
        self.region.as_ref()
    }

    pub fn get_config(&self) -> Arc<AgentConfig> {
        Arc::clone(&self.config)
    }
}
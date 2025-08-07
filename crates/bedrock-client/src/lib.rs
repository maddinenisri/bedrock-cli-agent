use aws_config::Region;
use aws_sdk_bedrockruntime as bedrock;
use bedrock_config::{AgentConfig, AwsSettings};
use bedrock_core::{BedrockError, Message, MessageRole, Result, TokenStatistics};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

pub struct BedrockClient {
    client: bedrock::Client,
    region: Region,
    config: Arc<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRequest {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationResponse {
    pub content: String,
    pub role: MessageRole,
    pub usage: TokenUsage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
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

    pub async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse> {
        let mut bedrock_messages = Vec::new();
        
        for msg in &request.messages {
            let content = bedrock::types::ContentBlock::Text(msg.content.clone());
            let role = match msg.role {
                MessageRole::User => bedrock::types::ConversationRole::User,
                MessageRole::Assistant => bedrock::types::ConversationRole::Assistant,
                _ => continue,
            };
            
            let bedrock_msg = bedrock::types::Message::builder()
                .role(role)
                .content(content)
                .build()
                .map_err(|e| BedrockError::Unknown(e.to_string()))?;
            
            bedrock_messages.push(bedrock_msg);
        }

        let mut converse_request = self.client
            .converse()
            .model_id(request.model_id.clone())
            .set_messages(Some(bedrock_messages));

        if let Some(system_prompt) = request.system_prompt {
            let system_content = bedrock::types::SystemContentBlock::Text(system_prompt);
            converse_request = converse_request.system(system_content);
        }

        let inference_config = bedrock::types::InferenceConfiguration::builder()
            .max_tokens(request.max_tokens.unwrap_or(self.config.agent.max_tokens) as i32)
            .temperature(request.temperature.unwrap_or(self.config.agent.temperature))
            .build();

        converse_request = converse_request.inference_config(inference_config);

        let response = converse_request.send().await
            .map_err(|e| BedrockError::Unknown(format!("Bedrock API error: {}", e)))?;

        let content = response.output()
            .and_then(|output| match output {
                bedrock::types::ConverseOutput::Message(msg) => {
                    msg.content().first().and_then(|block| match block {
                        bedrock::types::ContentBlock::Text(text) => Some(text.clone()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .unwrap_or_default();

        let usage = response.usage()
            .map(|u| TokenUsage {
                input_tokens: u.input_tokens() as usize,
                output_tokens: u.output_tokens() as usize,
                total_tokens: u.total_tokens() as usize,
            })
            .unwrap_or_default();

        let stop_reason = Some(format!("{:?}", response.stop_reason()));

        Ok(ConversationResponse {
            content,
            role: MessageRole::Assistant,
            usage,
            stop_reason,
        })
    }

    pub fn get_region(&self) -> &str {
        self.region.as_ref()
    }

    pub fn get_config(&self) -> Arc<AgentConfig> {
        Arc::clone(&self.config)
    }
}

impl From<TokenUsage> for TokenStatistics {
    fn from(usage: TokenUsage) -> Self {
        TokenStatistics {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            cache_hits: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage_conversion() {
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
        };

        let stats: TokenStatistics = usage.into();
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
        assert_eq!(stats.total_tokens, 150);
    }
}
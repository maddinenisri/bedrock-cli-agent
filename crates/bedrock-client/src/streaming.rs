use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message, StopReason, ToolUseBlock,
    ConverseStreamOutput, TokenUsage,
};
use aws_smithy_types::Document;
use bedrock_core::{BedrockError, Result};
use std::collections::HashMap;
use std::io::{self, Write};
use tracing::{debug, warn};

use crate::{ConverseResponse, BedrockClient};

/// Process a streaming response and reconstruct the full message
pub async fn process_stream_with_response<E>(
    stream: impl tokio_stream::Stream<Item = std::result::Result<ConverseStreamOutput, E>>,
) -> Result<ConverseResponse> 
where
    E: std::fmt::Display,
{
    use tokio_stream::StreamExt;
    tokio::pin!(stream);
    let mut collected_content = Vec::new();
    let mut accumulated_text = String::new();
    let mut stop_reason = StopReason::EndTurn;
    let mut token_usage: Option<TokenUsage> = None;
    
    // For tool use accumulation
    let mut current_tool_name: Option<String> = None;
    let mut current_tool_id: Option<String> = None;
    let mut tool_input_json = String::new();
    
    // For filtering excessive newlines
    let mut last_char_was_newline = false;
    let mut consecutive_newlines = 0;
    
    while let Some(event) = stream.next().await {
        match event {
            Ok(event_result) => {
                let event_type = match &event_result {
                    ConverseStreamOutput::ContentBlockDelta(_) => "ContentBlockDelta",
                    ConverseStreamOutput::ContentBlockStart(_) => "ContentBlockStart", 
                    ConverseStreamOutput::ContentBlockStop(_) => "ContentBlockStop",
                    ConverseStreamOutput::MessageStart(_) => "MessageStart",
                    ConverseStreamOutput::MessageStop(_) => "MessageStop",
                    ConverseStreamOutput::Metadata(_) => "Metadata",
                    _ => "Unknown",
                };
                debug!("Stream event received: {}", event_type);
                
                match event_result {
                    ConverseStreamOutput::ContentBlockDelta(delta) => {
                        if let Some(delta) = delta.delta() {
                            if let Ok(text) = delta.as_text() {
                                // Filter excessive newlines in streaming output
                                let mut filtered_text = String::new();
                                for ch in text.chars() {
                                    if ch == '\n' {
                                        if !last_char_was_newline {
                                            consecutive_newlines = 1;
                                            last_char_was_newline = true;
                                            filtered_text.push(ch);
                                        } else {
                                            consecutive_newlines += 1;
                                            // Allow maximum 2 consecutive newlines
                                            if consecutive_newlines <= 2 {
                                                filtered_text.push(ch);
                                            }
                                        }
                                    } else {
                                        consecutive_newlines = 0;
                                        last_char_was_newline = false;
                                        filtered_text.push(ch);
                                    }
                                }
                                
                                print!("{filtered_text}");
                                io::stdout().flush().ok();
                                accumulated_text.push_str(text); // Keep original for response
                            } else if let Ok(tool_use) = delta.as_tool_use() {
                                // Accumulate tool input JSON as it streams
                                let input_chunk = tool_use.input();
                                debug!("Tool input chunk: '{}'", input_chunk);
                                tool_input_json.push_str(input_chunk);
                            }
                        }
                    }
                    ConverseStreamOutput::ContentBlockStart(start) => {
                        if let Some(start) = start.start() {
                            if let Ok(tool_use_start) = start.as_tool_use() {
                                println!("\n🛠️  Using tool: {}", tool_use_start.name());
                                debug!("Tool start detected: {} ({})", tool_use_start.name(), tool_use_start.tool_use_id());
                                current_tool_name = Some(tool_use_start.name().to_string());
                                current_tool_id = Some(tool_use_start.tool_use_id().to_string());
                                tool_input_json.clear();
                            }
                        }
                    }
                    ConverseStreamOutput::ContentBlockStop(_stop) => {
                        if let Some(tool_name) = &current_tool_name {
                            if let Some(tool_id) = &current_tool_id {
                                debug!("ContentBlockStop for tool: {}, accumulated input: '{}'", tool_name, tool_input_json);
                                
                                // Parse the accumulated JSON input
                                let input_doc = if !tool_input_json.is_empty() {
                                    match serde_json::from_str::<serde_json::Value>(&tool_input_json) {
                                        Ok(input_value) => {
                                            debug!("Parsed input value: {:?}", input_value);
                                            BedrockClient::json_to_document(&input_value)?
                                        }
                                        Err(e) => {
                                            debug!("Failed to parse JSON: {}", e);
                                            return Err(BedrockError::Unknown(format!("Tool input JSON parsing error: {e}")));
                                        }
                                    }
                                } else {
                                    debug!("Tool {} has no input JSON", tool_name);
                                    Document::Object(HashMap::new())
                                };

                                // Create a ToolUseBlock for the collected content
                                let tool_use_block = ToolUseBlock::builder()
                                    .tool_use_id(tool_id)
                                    .name(tool_name)
                                    .input(input_doc)
                                    .build()
                                    .map_err(|e| BedrockError::Unknown(format!("Failed to build tool use block: {e}")))?;

                                collected_content.push(ContentBlock::ToolUse(tool_use_block));
                            }
                            current_tool_name = None;
                            current_tool_id = None;
                            tool_input_json.clear();
                        } else if !accumulated_text.is_empty() {
                            // Add text content
                            collected_content.push(ContentBlock::Text(accumulated_text.clone()));
                            accumulated_text.clear();
                        }
                    }
                    ConverseStreamOutput::MessageStop(stop) => {
                        println!(); // New line after streaming
                        debug!("Streaming completed with stop reason: {:?}", stop.stop_reason());
                        
                        stop_reason = stop.stop_reason().clone();
                        
                        // Add any remaining text content
                        if !accumulated_text.is_empty() {
                            collected_content.push(ContentBlock::Text(accumulated_text.clone()));
                            accumulated_text.clear();
                        }
                        
                        // Don't break yet - metadata events come after MessageStop
                        debug!("MessageStop received, waiting for potential metadata events...");
                        
                        // Show waiting status if this was a tool use
                        if matches!(stop_reason, StopReason::ToolUse) {
                            println!("⏳ Executing tools...");
                        }
                    }
                    ConverseStreamOutput::Metadata(metadata) => {
                        debug!("Received metadata event in stream: {:?}", metadata);
                        if let Some(usage) = metadata.usage() {
                            debug!("Token usage - Input: {:?}, Output: {:?}", 
                                   usage.input_tokens(), usage.output_tokens());
                            // Capture token usage for the response
                            token_usage = Some(usage.clone());
                        } else {
                            debug!("Metadata event has no usage information");
                        }
                        
                        // Now we can break as we've received the metadata
                        debug!("Metadata received, ending stream processing");
                        break;
                    }
                    _ => {
                        // Handle other event types as needed
                        debug!("Received other stream event type: {:?}", event_result);
                    }
                }
            }
            Err(e) => {
                warn!("Stream error: {}", e);
                return Err(BedrockError::Unknown(format!("Stream error: {e}")));
            }
        }
    }

    // Build the message from collected content
    let message = Message::builder()
        .role(ConversationRole::Assistant)
        .set_content(if collected_content.is_empty() {
            None
        } else {
            Some(collected_content)
        })
        .build()
        .map_err(|e| BedrockError::Unknown(format!("Failed to build message: {e}")))?;

    Ok(ConverseResponse {
        message,
        stop_reason,
        usage: token_usage,
    })
}


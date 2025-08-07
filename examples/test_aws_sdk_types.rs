use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use bedrock_client::{BedrockClient, ToolDefinition};
use bedrock_config::AgentConfig;
use bedrock_tools::ToolRegistry;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    println!("Testing AWS SDK Types Implementation...\n");

    // Load config
    let config = AgentConfig::from_yaml("config.yaml")?;
    
    // Create Bedrock client
    let client = Arc::new(BedrockClient::new(config.clone()).await?);
    
    // Create tool registry and add the execute_bash tool
    let tool_registry = Arc::new(ToolRegistry::with_default_tools("workspace"));
    
    // Test 1: Simple conversation without tools
    println!("Test 1: Simple conversation without tools");
    println!("==========================================");
    
    let user_msg = Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text("What is 2 + 2?".to_string()))
        .build()?;
    
    let response = client.converse(
        &config.agent.model,
        vec![user_msg],
        Some("You are a helpful assistant.".to_string()),
        None,
    ).await?;
    
    println!("Response: {}", response.get_text_content());
    println!("Stop reason: {:?}", response.stop_reason);
    println!();
    
    // Test 2: Conversation with tools
    println!("Test 2: Conversation with tools");
    println!("================================");
    
    // Build tool definitions
    let tool_definitions: Vec<ToolDefinition> = tool_registry
        .get_all()
        .into_iter()
        .map(|tool| ToolDefinition {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            input_schema: tool.schema(),
        })
        .collect();
    
    let user_msg = Message::builder()
        .role(ConversationRole::User)
        .content(ContentBlock::Text(
            "Run the command 'echo Hello from AWS SDK Types' using bash".to_string()
        ))
        .build()?;
    
    let mut conversation = vec![user_msg];
    
    // First call - should trigger tool use
    let response = client.converse(
        &config.agent.model,
        conversation.clone(),
        Some("You are a helpful assistant with access to bash commands.".to_string()),
        Some(tool_definitions.clone()),
    ).await?;
    
    println!("Initial response: {}", response.get_text_content());
    println!("Stop reason: {:?}", response.stop_reason);
    println!("Has tool use: {}", response.has_tool_use());
    
    // Add assistant response to conversation
    conversation.push(response.message.clone());
    
    if response.has_tool_use() {
        println!("\nExecuting tools...");
        let tool_uses = response.get_tool_uses();
        println!("Found {} tool calls", tool_uses.len());
        
        // Execute tools
        let tool_results = client.execute_tools(&tool_uses, &tool_registry).await?;
        println!("Executed {} tools", tool_results.len());
        
        // Create tool result message
        let tool_result_msg = Message::builder()
            .role(ConversationRole::User)
            .set_content(Some(
                tool_results
                    .into_iter()
                    .map(ContentBlock::ToolResult)
                    .collect(),
            ))
            .build()?;
        
        conversation.push(tool_result_msg);
        
        // Continue conversation with tool results
        let final_response = client.converse(
            &config.agent.model,
            conversation,
            Some("You are a helpful assistant with access to bash commands.".to_string()),
            Some(tool_definitions),
        ).await?;
        
        println!("\nFinal response: {}", final_response.get_text_content());
        println!("Stop reason: {:?}", final_response.stop_reason);
    }
    
    println!("\n✅ All tests completed successfully!");
    
    Ok(())
}
use std::sync::Arc;

use async_trait::async_trait;
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
use bedrock_client::{BedrockClient, ConverseResponse};
use bedrock_core::{BedrockError, Result};
use bedrock_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Represents a single todo item returned by the todo tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TodoItem {
    pub description: String,
}

/// Client abstraction for models capable of generating todo lists.
#[async_trait]
pub trait TodoModel: Send + Sync {
    async fn converse(
        &self,
        model_id: &str,
        messages: Vec<Message>,
        system_prompt: Option<String>,
    ) -> Result<ConverseResponse>;
}

#[async_trait]
impl TodoModel for BedrockClient {
    async fn converse(
        &self,
        model_id: &str,
        messages: Vec<Message>,
        system_prompt: Option<String>,
    ) -> Result<ConverseResponse> {
        BedrockClient::converse(self, model_id, messages, system_prompt, None).await
    }
}

/// Tool that calls a Bedrock model to generate todo items.
pub struct TodoTool {
    model_client: Arc<dyn TodoModel>,
    model: String,
}

impl TodoTool {
    pub fn new(model_client: Arc<dyn TodoModel>, model: impl Into<String>) -> Self {
        Self { model_client, model: model.into() }
    }
}

#[async_trait]
impl bedrock_tools::Tool for TodoTool {
    fn name(&self) -> &str { "todo" }

    fn description(&self) -> &str {
        "Generate a structured list of todo items for a given prompt"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Task description to plan into todo items",
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| BedrockError::ToolError {
                tool: "todo".to_string(),
                message: "missing prompt".to_string(),
            })?;

        let user_message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()
            .map_err(|e| BedrockError::Unknown(e.to_string()))?;

        let system_prompt = "You are an expert planning assistant. Break the user's request into a list of todo items and respond in JSON format: {\"items\":[{\"description\":\"...\"}]}".to_string();

        let response = self
            .model_client
            .converse(&self.model, vec![user_message], Some(system_prompt))
            .await?;

        let text = response.get_text_content();
        let value: Value = serde_json::from_str(&text).map_err(|e| BedrockError::ToolError {
            tool: "todo".to_string(),
            message: format!("Invalid JSON response: {e}"),
        })?;

        Ok(value)
    }
}

/// Plans todo items for a given prompt using the registered `todo` tool.
pub struct TodoPlanner {
    tool_registry: Arc<ToolRegistry>,
}

impl TodoPlanner {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    /// Calls the `todo` tool with the full task prompt and parses its JSON
    /// response into a list of [`TodoItem`]s.
    pub async fn plan(&self, prompt: &str) -> Result<Vec<TodoItem>> {
        let tool = self
            .tool_registry
            .get("todo")
            .ok_or_else(|| BedrockError::ToolError {
                tool: "todo".to_string(),
                message: "todo tool not registered".to_string(),
            })?;

        let response = tool
            .execute(serde_json::json!({ "prompt": prompt }))
            .await?;

        let items_value: Value = if let Some(items) = response.get("items") {
            items.clone()
        } else {
            response
        };

        let todos: Vec<TodoItem> = serde_json::from_value(items_value)?;
        Ok(todos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use aws_sdk_bedrockruntime::types::{
        ContentBlock, ConversationRole, Message, StopReason,
    };
    use bedrock_client::ConverseResponse;
    use bedrock_core::Result;
    use bedrock_tools::Tool; // bring trait into scope for tests
    use mockall::{mock, predicate::*};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    struct MockTodoTool {
        captured_prompt: Arc<Mutex<Option<String>>>,
        response: Value,
    }

    #[async_trait]
    impl bedrock_tools::Tool for MockTodoTool {
        fn name(&self) -> &str {
            "todo"
        }

        fn description(&self) -> &str {
            "mock todo tool"
        }

        fn schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {"prompt": {"type": "string"}}
            })
        }

        async fn execute(&self, args: Value) -> Result<Value> {
            if let Some(p) = args.get("prompt").and_then(|v| v.as_str()) {
                *self.captured_prompt.lock().unwrap() = Some(p.to_string());
            }
            Ok(self.response.clone())
        }
    }

    fn planner_with_tool(response: Value, captured: Arc<Mutex<Option<String>>>) -> TodoPlanner {
        let registry = ToolRegistry::new();
        let tool = MockTodoTool {
            captured_prompt: captured.clone(),
            response,
        };
        registry.register(tool).unwrap();
        TodoPlanner::new(Arc::new(registry))
    }

    mock! {
        pub Model {}

        #[async_trait]
        impl TodoModel for Model {
            async fn converse(
                &self,
                model_id: &str,
                messages: Vec<Message>,
                system_prompt: Option<String>,
            ) -> Result<ConverseResponse>;
        }
    }

    #[tokio::test]
    async fn plan_parses_multiple_sentences() {
        let captured = Arc::new(Mutex::new(None));
        let planner = planner_with_tool(
            json!([
                {"description": "install dependencies"},
                {"description": "run tests"}
            ]),
            captured.clone(),
        );

        let prompt = "Install dependencies. Then run tests.";
        let todos = planner.plan(prompt).await.unwrap();

        assert_eq!(todos.len(), 2);
        assert_eq!(captured.lock().unwrap().as_deref(), Some(prompt));
    }

    #[tokio::test]
    async fn plan_handles_complex_instructions() {
        let captured = Arc::new(Mutex::new(None));
        let planner = planner_with_tool(
            json!({"items": [
                {"description": "research requirement"},
                {"description": "implement feature"},
                {"description": "write integration tests"}
            ]}),
            captured.clone(),
        );

        let prompt = "Research requirement A, implement feature B, then write integration tests.";
        let todos = planner.plan(prompt).await.unwrap();

        assert_eq!(todos.len(), 3);
        assert_eq!(todos[0].description, "research requirement");
        assert_eq!(captured.lock().unwrap().as_deref(), Some(prompt));
    }

    #[tokio::test]
    async fn todo_tool_calls_model_and_parses_json() {
        let mut mock = MockModel::new();
        mock.expect_converse()
            .withf(|model_id, messages, system_prompt| {
                model_id == "model-id" && messages.len() == 1 && system_prompt.is_some()
            })
            .returning(|_, _, _| {
                let message = Message::builder()
                    .role(ConversationRole::Assistant)
                    .content(ContentBlock::Text("{\"items\":[{\"description\":\"a\"},{\"description\":\"b\"}]}".into()))
                    .build()
                    .unwrap();
                Ok(ConverseResponse { message, stop_reason: StopReason::EndTurn, usage: None })
            });

        let tool = TodoTool::new(Arc::new(mock), "model-id");
        let result = tool.execute(json!({"prompt": "Do something"})).await.unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), 2);
    }
}

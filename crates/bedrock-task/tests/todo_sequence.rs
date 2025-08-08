use std::sync::{Arc, Mutex};

use bedrock_task::{TaskExecutor, TodoItem};
use bedrock_client::BedrockClient;
use bedrock_config::AgentConfig;
use bedrock_tools::ToolRegistry;
use bedrock_core::{Result, BedrockError};

async fn build_executor() -> TaskExecutor {
    std::env::set_var("AWS_EC2_METADATA_DISABLED", "true");
    let yaml = r#"
agent:
  name: test
  model: test-model
aws:
  region: us-east-1
tools:
  allowed: []
pricing: {}
"#;
    let config = AgentConfig::from_yaml_str(yaml).unwrap();
    let client = BedrockClient::new(config.clone()).await.unwrap();
    TaskExecutor::new(
        Arc::new(client),
        Arc::new(ToolRegistry::new()),
        Arc::new(config),
    )
}

#[tokio::test]
async fn appends_new_todos_during_execution() {
    let executor = build_executor().await;
    let executed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let exec_executed = executed.clone();

    let execute = move |todo: &TodoItem| -> Result<()> {
        exec_executed
            .lock()
            .unwrap()
            .push(todo.description.clone());
        Ok(())
    };

    let mut planner_calls = 0;
    let planner = move |_: &[TodoItem]| -> Result<Vec<TodoItem>> {
        planner_calls += 1;
        if planner_calls == 1 {
            Ok(vec![TodoItem::new("second")])
        } else {
            Ok(vec![])
        }
    };

    let initial = vec![TodoItem::new("first")];
    let result = executor
        .execute_sequence(initial, execute, planner)
        .await
        .unwrap();

    assert_eq!(*executed.lock().unwrap(), vec!["first", "second"]);
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn planner_runs_after_error() {
    let executor = build_executor().await;
    let executed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let exec_executed = executed.clone();
    let mut first = true;

    let execute = move |todo: &TodoItem| -> Result<()> {
        exec_executed
            .lock()
            .unwrap()
            .push(todo.description.clone());
        if first {
            first = false;
            Err(BedrockError::TaskError("boom".into()))
        } else {
            Ok(())
        }
    };

    let mut planner_calls = 0;
    let planner = move |_: &[TodoItem]| -> Result<Vec<TodoItem>> {
        planner_calls += 1;
        if planner_calls == 1 {
            Ok(vec![TodoItem::new("second")])
        } else {
            Ok(vec![])
        }
    };

    let initial = vec![TodoItem::new("first")];
    let result = executor
        .execute_sequence(initial, execute, planner)
        .await
        .unwrap();

    assert_eq!(*executed.lock().unwrap(), vec!["first", "second"]);
    assert_eq!(result.len(), 2);
}

use bedrock_core::{Task, TodoItem};

pub struct TodoPlanner;

impl TodoPlanner {
    pub fn new() -> Self {
        Self
    }

    pub async fn plan(&self, task: &Task) -> Vec<TodoItem> {
        let mut todos = Vec::new();
        for part in task.prompt.split('.') {
            let desc = part.trim();
            if !desc.is_empty() {
                todos.push(TodoItem::new(desc));
            }
        }
        if todos.is_empty() {
            todos.push(TodoItem::new(task.prompt.clone()));
        }
        todos
    }
}

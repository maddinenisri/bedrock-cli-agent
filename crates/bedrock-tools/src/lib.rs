use async_trait::async_trait;
use bedrock_core::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub mod fs_tools;
pub mod search_tools;
pub mod execute_bash;
pub mod security;

pub use fs_tools::{FileReadTool, FileWriteTool, FileListTool};
pub use search_tools::{GrepTool, FindTool, RipgrepTool};
pub use execute_bash::ExecuteBashTool;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn execute(&self, args: Value) -> Result<Value>;
}

pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_default_tools(workspace_dir: impl Into<std::path::PathBuf>) -> Self {
        let registry = Self::new();
        let workspace = workspace_dir.into();
        
        // Register file system tools
        registry.register(FileReadTool::new(&workspace)).unwrap();
        registry.register(FileWriteTool::new(&workspace)).unwrap();
        registry.register(FileListTool::new(&workspace)).unwrap();
        
        // Register search tools
        registry.register(GrepTool::new(&workspace)).unwrap();
        registry.register(FindTool::new(&workspace)).unwrap();
        registry.register(RipgrepTool::new(&workspace)).unwrap();
        
        // Register execution tools
        registry.register(ExecuteBashTool::new(&workspace)).unwrap();
        
        registry
    }

    pub fn register(&self, tool: impl Tool + 'static) -> Result<()> {
        let mut tools = self.tools.write().unwrap();
        let name = tool.name().to_string();
        tools.insert(name, Arc::new(tool));
        Ok(())
    }

    pub fn unregister(&self, name: &str) -> Result<()> {
        let mut tools = self.tools.write().unwrap();
        tools.remove(name);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        let tools = self.tools.read().unwrap();
        tools.keys().cloned().collect()
    }
    
    pub fn get_all(&self) -> Vec<Arc<dyn Tool>> {
        let tools = self.tools.read().unwrap();
        tools.values().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum Permission {
    Allow,
    Ask,
    Deny,
}

pub struct PermissionPolicy {
    pub tool_name: String,
    pub permission: Permission,
}

pub struct PermissionManager {
    policies: HashMap<String, PermissionPolicy>,
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    pub fn add_policy(&mut self, policy: PermissionPolicy) {
        self.policies.insert(policy.tool_name.clone(), policy);
    }

    pub fn check(&self, tool_name: &str) -> Permission {
        self.policies
            .get(tool_name)
            .map(|p| p.permission.clone())
            .unwrap_or(Permission::Ask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(&self, _args: Value) -> Result<Value> {
            Ok(json!({"result": "success"}))
        }
    }

    #[test]
    fn test_tool_registry() {
        let registry = ToolRegistry::new();
        let tool = MockTool {
            name: "test_tool".to_string(),
        };

        registry.register(tool).unwrap();
        assert!(registry.get("test_tool").is_some());
        assert_eq!(registry.list().len(), 1);

        registry.unregister("test_tool").unwrap();
        assert!(registry.get("test_tool").is_none());
    }
    
    #[test]
    fn test_default_tools() {
        let registry = ToolRegistry::with_default_tools("/tmp");
        let tools = registry.list();
        
        assert!(tools.contains(&"fs_read".to_string()));
        assert!(tools.contains(&"fs_write".to_string()));
        assert!(tools.contains(&"fs_list".to_string()));
        assert!(tools.contains(&"grep".to_string()));
        assert!(tools.contains(&"find".to_string()));
        assert!(tools.contains(&"rg".to_string()));
        
        // Check for execute_bash/execute_cmd based on OS
        if cfg!(target_os = "windows") {
            assert!(tools.contains(&"execute_cmd".to_string()));
        } else {
            assert!(tools.contains(&"execute_bash".to_string()));
        }
    }
}
# MCP Implementation Improvement Plan

## Overview
This document provides a detailed, actionable plan to fix the critical MCP implementation issues and align with AWS Bedrock best practices.

## Critical Fix #1: Tool Interface Alignment

### Problem
The current `McpToolWrapper` doesn't match the `bedrock-tools::Tool` trait interface, causing compilation/runtime errors.

### Solution
Update the tool wrapper to properly implement the Tool trait with AWS Document types:

```rust
// File: crates/bedrock-mcp/src/tool_wrapper.rs

use async_trait::async_trait;
use bedrock_core::Result;
use bedrock_tools::Tool;
use aws_smithy_types::Document;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

use crate::client::McpClient;
use crate::types::{ContentItem, McpTool};

/// Helper function to convert AWS Document to JSON Value
fn document_to_json(doc: &Document) -> Result<Value> {
    match doc {
        Document::Object(map) => {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                json_map.insert(k.clone(), document_to_json(v)?);
            }
            Ok(Value::Object(json_map))
        }
        Document::Array(arr) => {
            let json_arr: Result<Vec<Value>> = arr.iter().map(document_to_json).collect();
            Ok(Value::Array(json_arr?))
        }
        Document::Number(n) => {
            if let Ok(i) = n.as_i64() {
                Ok(json!(i))
            } else if let Ok(f) = n.as_f64() {
                Ok(json!(f))
            } else {
                Ok(json!(n.to_string()))
            }
        }
        Document::String(s) => Ok(Value::String(s.clone())),
        Document::Bool(b) => Ok(Value::Bool(*b)),
        Document::Null => Ok(Value::Null),
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn schema(&self) -> Value {
        self.tool_def.input_schema.clone()
    }

    async fn execute(&self, input: &Document) -> Result<Value> {
        debug!(
            "Executing MCP tool '{}' from server '{}'",
            self.tool_def.name, self.server_name
        );
        
        // Convert Document to JSON for MCP protocol
        let input_json = document_to_json(input)?;
        
        // Call the tool through MCP client
        let mut client = self.client.write().await;
        match client.call_tool(&self.tool_def.name, input_json).await {
            Ok(content_items) => {
                // Process content items into response
                process_content_items(content_items, &self.server_name, &self.tool_def.name)
            }
            Err(e) => {
                error!("MCP tool execution failed: {}", e);
                Ok(json!({
                    "error": e.to_string(),
                    "success": false,
                    "server": self.server_name,
                    "tool": self.tool_def.name
                }))
            }
        }
    }
}

fn process_content_items(
    content_items: Vec<ContentItem>,
    server_name: &str,
    tool_name: &str
) -> Result<Value> {
    let mut text_content = Vec::new();
    let mut images = Vec::new();
    
    for item in content_items {
        match item {
            ContentItem::Text { text } => {
                text_content.push(text);
            }
            ContentItem::Image { data, mime_type } => {
                images.push(json!({
                    "type": "image",
                    "data": data,
                    "mime_type": mime_type
                }));
            }
        }
    }
    
    // Return simple text for single text response, structured otherwise
    if text_content.len() == 1 && images.is_empty() {
        Ok(json!(text_content[0]))
    } else {
        let mut response = json!({
            "success": true,
            "server": server_name,
            "tool": tool_name
        });
        
        if !text_content.is_empty() {
            response["content"] = json!(text_content.join("\n"));
        }
        
        if !images.is_empty() {
            response["images"] = json!(images);
        }
        
        Ok(response)
    }
}
```

## Critical Fix #2: Add AWS Type Conversion Utilities

### Create Type Conversion Module
```rust
// File: crates/bedrock-mcp/src/conversions.rs

use aws_smithy_types::Document;
use bedrock_core::{BedrockError, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Convert JSON Value to AWS Document with depth protection
pub fn json_to_document(value: &Value) -> Result<Document> {
    json_to_document_with_depth(value, 0)
}

fn json_to_document_with_depth(value: &Value, depth: usize) -> Result<Document> {
    const MAX_DEPTH: usize = 100;
    
    if depth > MAX_DEPTH {
        return Ok(Document::String(
            format!("[Deep nested object at depth {}]", depth)
        ));
    }
    
    match value {
        Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_document_with_depth(v, depth + 1)?);
            }
            Ok(Document::Object(map))
        }
        Value::Array(arr) => {
            let docs: Result<Vec<Document>> = arr
                .iter()
                .map(|v| json_to_document_with_depth(v, depth + 1))
                .collect();
            Ok(Document::Array(docs?))
        }
        Value::String(s) => Ok(Document::String(s.clone())),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Document::Number(aws_smithy_types::Number::NegInt(i)))
            } else if let Some(u) = n.as_u64() {
                Ok(Document::Number(aws_smithy_types::Number::PosInt(u)))
            } else if let Some(f) = n.as_f64() {
                Ok(Document::Number(aws_smithy_types::Number::Float(f)))
            } else {
                Err(BedrockError::SerializationError(
                    serde_json::Error::custom("Invalid number")
                ))
            }
        }
        Value::Bool(b) => Ok(Document::Bool(*b)),
        Value::Null => Ok(Document::Null),
    }
}

/// Convert AWS Document to JSON Value
pub fn document_to_json(doc: &Document) -> Result<Value> {
    match doc {
        Document::Object(map) => {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                json_map.insert(k.clone(), document_to_json(v)?);
            }
            Ok(Value::Object(json_map))
        }
        Document::Array(arr) => {
            let json_arr: Result<Vec<Value>> = arr.iter().map(document_to_json).collect();
            Ok(Value::Array(json_arr?))
        }
        Document::Number(n) => match n {
            aws_smithy_types::Number::PosInt(i) => Ok(json!(i)),
            aws_smithy_types::Number::NegInt(i) => Ok(json!(i)),
            aws_smithy_types::Number::Float(f) => Ok(json!(f)),
        },
        Document::String(s) => Ok(Value::String(s.clone())),
        Document::Bool(b) => Ok(Value::Bool(*b)),
        Document::Null => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_document_conversion() {
        let json = json!({
            "name": "test",
            "count": 42,
            "active": true,
            "items": ["a", "b", "c"],
            "nested": {
                "value": 3.14
            }
        });
        
        let doc = json_to_document(&json).unwrap();
        let back = document_to_json(&doc).unwrap();
        
        assert_eq!(json, back);
    }
    
    #[test]
    fn test_depth_protection() {
        let mut deeply_nested = json!({});
        let mut current = &mut deeply_nested;
        
        // Create deeply nested structure
        for _ in 0..150 {
            *current = json!({"nested": {}});
            current = current.get_mut("nested").unwrap();
        }
        
        // Should not panic, should handle gracefully
        let result = json_to_document(&deeply_nested);
        assert!(result.is_ok());
    }
}
```

## Critical Fix #3: Simplify Client Response Handling

### Refactor Client for Direct Response Correlation
```rust
// File: crates/bedrock-mcp/src/client.rs (updated sections)

use tokio::time::{timeout, Duration};

impl McpClient {
    /// Send request and wait for response with direct correlation
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let request_id = request.id.clone();
        
        // Send the request
        {
            let mut transport = self.transport.write().await;
            transport.send_request(request).await?;
        }
        
        // Wait for response with timeout and correlation
        let timeout_duration = Duration::from_millis(
            self.config.timeout().unwrap_or(30000)
        );
        
        timeout(timeout_duration, self.wait_for_response(request_id)).await
            .map_err(|_| BedrockError::McpError("Request timed out".into()))?
    }
    
    /// Wait for a specific response by ID
    async fn wait_for_response(&mut self, request_id: String) -> Result<JsonRpcResponse> {
        let start = std::time::Instant::now();
        let max_wait = Duration::from_millis(30000);
        
        loop {
            // Check for timeout
            if start.elapsed() > max_wait {
                return Err(BedrockError::McpError(
                    format!("Timeout waiting for response to request {}", request_id)
                ));
            }
            
            // Try to receive response
            let mut transport = self.transport.write().await;
            if let Some(response) = transport.receive_response().await? {
                if response.id == request_id {
                    return Ok(response);
                }
                // If not our response, log and continue
                debug!("Received response for different request: {}", response.id);
            }
            
            // Release lock and wait briefly
            drop(transport);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
```

## Implementation Timeline

### Week 1: Critical Fixes
**Day 1-2:**
- [ ] Update Tool trait implementation in tool_wrapper.rs
- [ ] Add conversions.rs module with type conversion helpers
- [ ] Update lib.rs to export conversion functions

**Day 3-4:**
- [ ] Simplify client response handling
- [ ] Remove complex channel-based mechanism
- [ ] Add request correlation logic

**Day 5:**
- [ ] Integration testing with real MCP servers
- [ ] Fix any runtime issues discovered

### Week 2: Architecture Improvements
**Day 1-2:**
- [ ] Add tool caching to McpClient
- [ ] Implement hierarchical configuration loading
- [ ] Add standard config path support

**Day 3-4:**
- [ ] Improve error context and types
- [ ] Add retry logic with exponential backoff
- [ ] Implement connection pooling

**Day 5:**
- [ ] Performance testing and optimization
- [ ] Documentation updates

### Week 3: Production Features
**Day 1-2:**
- [ ] Add comprehensive health monitoring
- [ ] Implement circuit breaker patterns
- [ ] Add metrics collection

**Day 3-4:**
- [ ] Security hardening (input validation, sanitization)
- [ ] Secret management integration
- [ ] Rate limiting implementation

**Day 5:**
- [ ] End-to-end testing with AWS Bedrock
- [ ] Performance benchmarking

### Week 4: Polish and Documentation
**Day 1-2:**
- [ ] Add observability (OpenTelemetry integration)
- [ ] Implement distributed tracing
- [ ] Add structured logging

**Day 3-4:**
- [ ] Create migration guide for existing users
- [ ] Update all documentation
- [ ] Add comprehensive examples

**Day 5:**
- [ ] Final testing and bug fixes
- [ ] Release preparation

## Testing Strategy

### Unit Tests
```rust
// Example test for type conversion
#[test]
fn test_mcp_tool_wrapper_with_document() {
    let doc = Document::Object(HashMap::from([
        ("path".to_string(), Document::String("/tmp/test.txt".to_string())),
    ]));
    
    let result = wrapper.execute(&doc).await.unwrap();
    assert!(result.is_object());
}
```

### Integration Tests
```bash
# Test with real MCP servers
cargo test --features integration-tests

# Test with AWS Bedrock
cargo test --features bedrock-integration
```

### Performance Tests
```rust
#[bench]
fn bench_document_conversion(b: &mut Bencher) {
    let large_json = generate_large_json();
    b.iter(|| {
        let doc = json_to_document(&large_json).unwrap();
        let _ = document_to_json(&doc).unwrap();
    });
}
```

## Success Criteria

1. **Functional**: All MCP tools work correctly with AWS Bedrock
2. **Performance**: < 10ms overhead for tool execution
3. **Reliability**: 99.9% uptime with health monitoring
4. **Security**: Pass security audit with no critical issues
5. **Maintainability**: 90%+ test coverage

## Risk Mitigation

### Risk 1: Breaking Changes
- **Mitigation**: Version the changes, provide migration guide
- **Fallback**: Keep old implementation available via feature flag

### Risk 2: Performance Regression
- **Mitigation**: Benchmark before/after each change
- **Fallback**: Revert specific optimizations if needed

### Risk 3: MCP Server Compatibility
- **Mitigation**: Test with multiple MCP server implementations
- **Fallback**: Add compatibility shims for specific servers

## Conclusion

This improvement plan addresses all critical issues identified in the gap analysis. Following this plan will result in a robust, production-ready MCP implementation that properly integrates with AWS Bedrock runtime and follows best practices from the reference implementation.
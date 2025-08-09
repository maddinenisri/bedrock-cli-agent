//! Type conversion utilities for MCP integration
//! 
//! Provides conversion between AWS Document types and JSON values,
//! though currently not needed since bedrock-tools uses Value directly.

use bedrock_core::Result;
use serde_json::Value;

/// Helper to ensure JSON values are properly formatted for MCP
/// This can be extended if we need AWS Document conversion in the future
pub fn validate_json_for_mcp(value: &Value) -> Result<Value> {
    // For now, just pass through - but this gives us a place to add
    // validation or conversion logic if needed
    Ok(value.clone())
}

/// Process MCP response into a format Bedrock tools expect
pub fn process_mcp_response(text_content: Vec<String>, images: Vec<Value>) -> Value {
    // AWS Bedrock requires tool results to always be JSON objects, never plain strings
    let mut response = serde_json::json!({
        "success": true
    });
    
    if !text_content.is_empty() {
        // Always wrap content in an object for Bedrock compatibility
        response["content"] = Value::String(text_content.join("\n"));
    }
    
    if !images.is_empty() {
        response["images"] = Value::Array(images);
    }
    
    response
}

/// Reserved for future AWS Document conversion if needed
/// Currently bedrock-tools uses Value directly, so this is not used
#[allow(dead_code)]
pub fn json_to_document_stub(value: &Value) -> Result<Value> {
    // Placeholder for future AWS Document conversion
    // For now, just validate and return the JSON
    validate_json_for_mcp(value)
}

/// Reserved for future AWS Document conversion if needed
#[allow(dead_code)]
pub fn document_to_json_stub(value: &Value) -> Result<Value> {
    // Placeholder for future AWS Document conversion
    // For now, just pass through
    Ok(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_process_single_text_response() {
        let text = vec!["Hello, world!".to_string()];
        let images = vec![];
        
        let result = process_mcp_response(text, images);
        assert!(result.is_object());
        assert_eq!(result["success"], Value::Bool(true));
        assert_eq!(result["content"], Value::String("Hello, world!".to_string()));
    }
    
    #[test]
    fn test_process_multiple_text_response() {
        let text = vec!["Line 1".to_string(), "Line 2".to_string()];
        let images = vec![];
        
        let result = process_mcp_response(text, images);
        assert!(result.is_object());
        assert_eq!(result["content"], Value::String("Line 1\nLine 2".to_string()));
    }
    
    #[test]
    fn test_process_mixed_content_response() {
        let text = vec!["Description".to_string()];
        let images = vec![json!({
            "type": "image",
            "data": "base64data",
            "mime_type": "image/png"
        })];
        
        let result = process_mcp_response(text, images);
        assert!(result.is_object());
        assert_eq!(result["content"], Value::String("Description".to_string()));
        assert!(result["images"].is_array());
    }
    
    #[test]
    fn test_validate_json() {
        let json = json!({
            "name": "test",
            "count": 42,
            "active": true
        });
        
        let result = validate_json_for_mcp(&json).unwrap();
        assert_eq!(json, result);
    }
}
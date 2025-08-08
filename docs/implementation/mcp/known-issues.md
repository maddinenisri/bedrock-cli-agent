# MCP Implementation - Known Issues and Priority Fixes

## 🚨 Critical Issues (Blocking Production Use)

### Issue #1: Tool Interface Incompatibility
**Severity**: CRITICAL - Prevents all MCP tool execution  
**Component**: `crates/bedrock-mcp/src/tool_wrapper.rs`

#### Problem
The `McpToolWrapper` doesn't implement the correct `Tool` trait from `bedrock-tools`:

```rust
// Current implementation (WRONG):
#[async_trait]
impl Tool for McpToolWrapper {
    async fn execute(&self, args: Value) -> Result<Value> { ... }
}

// Required by bedrock-tools (CORRECT):
#[async_trait]
impl Tool for McpToolWrapper {
    async fn execute(&self, input: &Document) -> Result<Value> { ... }
}
```

#### Impact
- Compilation or runtime errors when registering MCP tools
- Cannot execute any MCP tools through Bedrock
- Blocks all MCP functionality

#### Fix Required
```rust
// In tool_wrapper.rs
#[async_trait]
impl Tool for McpToolWrapper {
    async fn execute(&self, input: &Document) -> Result<Value> {
        // Convert Document to JSON
        let args = document_to_json(input)?;
        
        // Execute MCP tool
        let result = self.client.call_tool(&self.tool.name, args).await?;
        
        // Return as Value
        Ok(result)
    }
}
```

---

### Issue #2: Missing Document Type Conversion
**Severity**: CRITICAL - Cannot process AWS Bedrock inputs  
**Component**: Missing helper functions

#### Problem
No implementation for converting between AWS `Document` and JSON `Value` types:

```rust
// MISSING: These functions don't exist
fn document_to_json(doc: &Document) -> Result<Value>
fn json_to_document(value: &Value) -> Result<Document>
```

#### Impact
- Cannot convert tool inputs from Bedrock (Document) to MCP (JSON)
- Cannot convert tool results from MCP (JSON) to Bedrock (Document)
- Blocks all data flow between Bedrock and MCP

#### Fix Required
```rust
// Add to crates/bedrock-mcp/src/conversions.rs (new file)
use aws_smithy_types::Document;
use serde_json::Value;

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
            let json_arr: Result<Vec<Value>> = arr.iter()
                .map(document_to_json)
                .collect();
            Ok(Value::Array(json_arr?))
        }
        Document::Number(n) => {
            // Handle number conversion
            Ok(json!(n.as_f64().unwrap_or_default()))
        }
        Document::String(s) => Ok(Value::String(s.clone())),
        Document::Bool(b) => Ok(Value::Bool(*b)),
        Document::Null => Ok(Value::Null),
    }
}

pub fn json_to_document(value: &Value) -> Result<Document> {
    // Implement reverse conversion with depth protection
}
```

---

### Issue #3: Complex Response Handling Architecture
**Severity**: HIGH - Performance and reliability issues  
**Component**: `crates/bedrock-mcp/src/client.rs`

#### Problem
Current implementation uses unnecessarily complex channel-based response correlation:

```rust
// Current: Complex with potential issues
pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>
// Spawned background task for response handling
```

#### Impact
- Added latency from channel communication
- Potential memory leaks if requests aren't cleaned up
- Difficult to debug and maintain
- Race conditions possible

#### Fix Required
- Simplify to direct response handling
- Remove spawned background tasks
- Implement proper cleanup on timeouts
- Add request ID tracking

---

## ⚠️ High Priority Issues

### Issue #4: No Connection Pooling
**Severity**: HIGH - Performance impact  
**Component**: Transport layer

#### Problem
- Creates new connections for each server
- No connection reuse
- No load balancing

#### Impact
- Higher latency
- Resource waste
- Scaling limitations

---

### Issue #5: Inadequate Error Context
**Severity**: MEDIUM - Debugging difficulty  
**Component**: All error paths

#### Problem
- Generic error wrapping loses context
- Missing server identification in errors
- No request correlation in error messages

#### Impact
- Difficult to debug production issues
- Poor error messages for users
- Hard to trace failure sources

---

## 📋 Medium Priority Issues

### Issue #6: Missing Health Check Integration
**Severity**: MEDIUM  
**Component**: `McpManager`

#### Problem
- Health checks run separately from client
- No integration with circuit breaker patterns
- Basic retry without backoff strategies

---

### Issue #7: No Tool Result Caching
**Severity**: MEDIUM  
**Component**: `McpToolWrapper`

#### Problem
- Every tool call goes to MCP server
- No caching of idempotent operations
- Missing cache invalidation strategy

---

## 🔧 Low Priority Enhancements

### Issue #8: Configuration Limitations
- No hierarchical config loading
- Missing standard config paths
- No integration with AWS Secrets Manager

### Issue #9: Observability Gaps
- Limited metrics collection
- No distributed tracing support
- Missing performance profiling

### Issue #10: Test Coverage
- No end-to-end integration tests
- Missing AWS Bedrock mock tests
- Limited error scenario coverage

---

## Priority Fix Roadmap

### Phase 1: Critical Fixes (Must Fix Immediately)
1. **Fix Tool trait implementation** - Without this, nothing works
2. **Add Document conversion helpers** - Required for AWS integration
3. **Update tool_wrapper.rs** - Use new conversions

### Phase 2: Stabilization (1-2 days)
1. **Simplify response handling** - Remove complex channels
2. **Add proper error context** - Improve debugging
3. **Fix memory leaks** - Clean up pending requests

### Phase 3: Production Readiness (3-5 days)
1. **Add connection pooling** - Improve performance
2. **Implement health monitoring** - Production reliability
3. **Add comprehensive tests** - Ensure stability

### Phase 4: Optimization (1 week)
1. **Add caching layer** - Reduce latency
2. **Implement circuit breakers** - Fault tolerance
3. **Add metrics and tracing** - Observability

---

## Testing Required After Fixes

### Critical Path Tests
```bash
# Test tool execution with AWS types
cargo test -p bedrock-mcp test_tool_execution

# Test Document conversion
cargo test -p bedrock-mcp test_document_conversion

# Integration test with real MCP server
cargo run --example test_mcp_bedrock_integration
```

### Validation Checklist
- [ ] MCP tools register successfully
- [ ] Tool execution works with Document input
- [ ] Response handling doesn't leak memory
- [ ] Error messages include context
- [ ] Health checks detect failures
- [ ] Configuration loads correctly

---

## Workarounds (Until Fixed)

### Cannot Use MCP Tools
**Workaround**: Use built-in tools only
```yaml
tools:
  allowed:
    - fs_read
    - fs_write
    - grep
    # Don't include MCP tools
```

### If You Must Try MCP
**Warning**: Will not work properly, but to test connection:
```yaml
mcp:
  enabled: false  # Keep disabled in production
```

---

## References

- [MCP Specification](https://modelcontextprotocol.io/docs)
- [AWS SDK Rust Documentation](https://docs.rs/aws-sdk-bedrockruntime)
- Original implementation: `rust-bedrock-api` project
- Gap analysis: `MCP_IMPLEMENTATION_GAPS.md`

---

**Last Updated**: Current as of repository state  
**Status**: BLOCKED - Requires critical fixes before use
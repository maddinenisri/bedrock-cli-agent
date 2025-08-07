# Cache Implementation Draft Plan - Issue #14

## Executive Summary
This document outlines a draft plan for implementing a caching layer for the Bedrock CLI Agent. The goal is to reduce API token consumption and costs by caching conversation responses.

## Critical Challenge: Tool Use and Unique IDs

### The Problem
When LLMs generate tool use requests, they create unique `tool_use_id` values for each tool invocation. This means:
- Even identical prompts will generate different tool_use_ids
- Tool results must reference these exact IDs
- This makes direct response caching nearly impossible for tool-using conversations

### Example
```
Prompt: "Write hello to test.txt"

Response 1: 
- tool_use_id: "toolu_01ABC123..."
- Tool result must reference: "toolu_01ABC123..."

Response 2 (same prompt):
- tool_use_id: "toolu_01XYZ789..." (different!)
- Tool result must reference: "toolu_01XYZ789..."
```

## Caching Strategies

### Strategy 1: Cache Only Non-Tool Responses
**Approach**: Only cache responses that don't involve tool use
- ✅ Simple to implement
- ✅ No ID mismatch issues
- ❌ Misses majority of use cases (most tasks use tools)
- ❌ Limited benefit

### Strategy 2: Semantic Response Caching
**Approach**: Cache the semantic intent and results, not the exact response
```rust
struct SemanticCache {
    // Cache the operation outcome, not the exact response
    operation: String,        // "write_file"
    parameters: Value,         // {"path": "test.txt", "content": "hello"}
    result: Value,            // {"success": true, "path": "test.txt"}
    final_message: String,    // "Successfully wrote hello to test.txt"
}
```
- ✅ Works with tool use
- ❌ Complex to implement
- ❌ Requires parsing and understanding intent
- ❌ May not match LLM's exact phrasing

### Strategy 3: Request-Level Caching with Tool Replay
**Approach**: Cache at the request level and replay tool executions
```rust
struct CachedRequest {
    prompt: String,
    tool_sequence: Vec<ToolExecution>,
    final_response: String,
}
```
When cache hit:
1. Skip LLM call
2. Re-execute tools with fresh IDs
3. Return cached final response

- ✅ Handles tool use
- ⚠️ Tools must be idempotent
- ❌ Side effects may differ (timestamps, file contents)

### Strategy 4: Hybrid Intelligent Caching
**Approach**: Combine multiple strategies based on request type
```rust
enum CacheStrategy {
    DirectResponse,     // No tools - cache full response
    ToolReplay,        // Deterministic tools - cache and replay
    SemanticResult,    // Non-deterministic - cache outcome
    NoCache,           // Dynamic content - don't cache
}
```

## Proposed Implementation (Phase 1 - Conservative)

### 1. Start with Non-Tool Response Caching
```rust
pub struct ResponseCache {
    // Only cache responses without tool use
    lru: Arc<RwLock<LruCache<String, CachedResponse>>>,
    cache_dir: PathBuf,
    stats: CacheStats,
}

impl ResponseCache {
    pub fn is_cacheable(response: &ConverseResponse) -> bool {
        // Don't cache if response contains tool use
        !response.has_tool_use()
    }
}
```

### 2. Cache Key Generation
```rust
fn generate_cache_key(
    model_id: &str,
    messages: &[Message],
    system_prompt: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    
    // Include model to prevent cross-model cache pollution
    hasher.update(model_id.as_bytes());
    
    // Hash message content (excluding tool_use_ids)
    for msg in messages {
        hasher.update(msg.role().as_str().as_bytes());
        // Extract and hash only the semantic content
        hasher.update(extract_content(msg).as_bytes());
    }
    
    if let Some(prompt) = system_prompt {
        hasher.update(prompt.as_bytes());
    }
    
    format!("{:x}", hasher.finalize())
}
```

### 3. Configuration Options
```yaml
cache:
  enabled: true
  strategies:
    non_tool_responses: true     # Phase 1
    deterministic_tools: false   # Future
    semantic_caching: false       # Future
  max_size: 1000
  ttl_seconds: 3600
  cache_dir: "${HOME_DIR:-~/.bedrock-agent}/cache"
```

## Implementation Phases

### Phase 1: Basic Non-Tool Caching (MVP)
- Cache responses without tool use
- Simple key generation
- Basic persistence
- Metrics collection

### Phase 2: Deterministic Tool Caching
- Identify deterministic tools (read operations)
- Implement tool replay mechanism
- Handle idempotent operations

### Phase 3: Semantic Caching
- Parse and understand operation intent
- Cache semantic results
- Generate appropriate responses

### Phase 4: Intelligent Strategy Selection
- Analyze request patterns
- Select optimal caching strategy
- Machine learning for cache prediction

## Metrics to Track

```rust
pub struct CacheMetrics {
    // Basic stats
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    
    // Categorized by type
    pub non_tool_hits: u64,
    pub tool_requests_skipped: u64,  // Couldn't cache due to tools
    
    // Savings
    pub tokens_saved: u64,
    pub cost_saved: f64,
    pub time_saved_ms: u64,
}
```

## Open Questions

1. **Tool Determinism**: How do we identify which tools are safe to cache?
   - File reads: Generally safe
   - File writes: Not safe (side effects)
   - Search operations: Depends on file system state

2. **Cache Invalidation**: When should we invalidate?
   - TTL-based expiration
   - File system changes
   - Manual invalidation
   - Configuration changes

3. **Memory vs Disk Trade-off**:
   - How much to keep in memory?
   - When to persist to disk?
   - Compression strategies?

4. **Privacy Concerns**:
   - Should we encrypt cached responses?
   - How to handle sensitive data?
   - User opt-out mechanisms?

## Alternative Approach: Conversation-Level Caching

Instead of caching individual responses, cache entire conversation flows:

```rust
pub struct ConversationCache {
    // Cache full conversation patterns
    patterns: HashMap<ConversationPattern, ConversationFlow>,
}

pub struct ConversationPattern {
    intent: String,           // "file_manipulation"
    operations: Vec<String>,  // ["write", "read", "verify"]
}
```

This could recognize patterns like:
- "Write X to file Y then read it back" 
- "Search for X and summarize"
- "List files and count them"

## Recommendations

1. **Start Conservative**: Begin with Phase 1 (non-tool caching) to prove value
2. **Measure Impact**: Collect metrics to understand usage patterns
3. **Iterate Based on Data**: Use metrics to guide next phases
4. **Consider Alternatives**: Explore conversation-level patterns
5. **User Control**: Allow users to disable/clear cache

## Example Use Cases That Would Benefit

### Cacheable (Phase 1):
- "What is 2+2?"
- "Explain concept X"
- "Translate this text"
- "Generate a poem about Y"

### Future Cacheable (Phase 2+):
- "Read file X" (deterministic)
- "List files in directory" (semi-deterministic)
- "Search for pattern Y" (depends on file system state)

### Never Cacheable:
- "Write current time to file"
- "Generate a random UUID"
- "Create a file with timestamp"

## Next Steps

1. Review and refine this approach
2. Prototype Phase 1 implementation
3. Benchmark token/cost savings
4. Gather user feedback
5. Plan Phase 2 based on learnings

## Conclusion

The tool use ID challenge significantly complicates caching for LLM responses. A phased approach starting with non-tool responses provides immediate value while we research more sophisticated strategies for tool-using conversations. The key insight is that we may need to cache at a higher semantic level rather than raw response level to handle the dynamic nature of tool use IDs.
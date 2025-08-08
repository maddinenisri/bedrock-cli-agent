# Bedrock CLI Agent - Architecture

## System Architecture Overview

The Bedrock CLI Agent is built on a modular, layered architecture designed for extensibility, security, and performance. The system follows clean architecture principles with clear separation of concerns between layers.

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Interface                         │
│                    (Command Processing)                      │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                      Agent Orchestrator                      │
│              (Task Management & Coordination)                │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┴─────────────────────┐
        │                                           │
┌───────▼────────┐                      ┌──────────▼──────────┐
│  Tool System   │                      │   Bedrock Client    │
│  (Execution)   │                      │   (LLM Interface)   │
└────────────────┘                      └───────────────────┘
        │                                           │
┌───────▼────────┐                      ┌──────────▼──────────┐
│ Tool Registry  │                      │    AWS SDK          │
│ (Management)   │                      │  (Communication)    │
└────────────────┘                      └───────────────────┘
```

## Core Components

### 1. CLI Layer (`src/main.rs`)

**Responsibility**: User interface and command processing

**Key Features**:
- Command parsing with Clap
- Argument validation
- Output formatting
- Interactive mode support

**Commands**:
```rust
enum Commands {
    Task { prompt: String, context: Option<String> },
    Chat { system: Option<String> },
    Test,
    Tools,
}
```

### 2. Agent Orchestrator (`crates/bedrock-agent`)

**Responsibility**: Central coordination and task execution

**Architecture**:
```
Agent
├── Task Queue (priority-based)
├── Tool Registry (thread-safe)
├── Bedrock Client (AWS integration)
├── Metrics Collector
└── MCP Manager (external tools)
```

**Key Components**:
- **Agent**: Main orchestrator implementing the `Agent` trait
- **Task Executor**: Manages task lifecycle and execution
- **Response Handler**: Processes LLM responses and tool calls
- **State Manager**: Maintains conversation context

### 3. Bedrock Client (`crates/bedrock-client`)

**Responsibility**: AWS Bedrock LLM communication

**Architecture**:
```
BedrockClient
├── AWS Client (SDK)
├── Conversation Manager
├── Streaming Handler
└── Response Parser
```

**Key Features**:
- Credential chain support (profile, env, IAM)
- Streaming response handling
- Tool call processing
- Token counting

**API Flow**:
```
1. Request → Build Conversation → Send to Bedrock
2. Response → Parse Content → Extract Tool Calls
3. Tool Execution → Format Results → Continue Conversation
4. Final Response → Return to User
```

### 4. Tool System (`crates/bedrock-tools`)

**Responsibility**: Extensible tool execution framework

**Architecture**:
```
Tool System
├── Tool Trait (interface)
├── Tool Registry (storage)
├── Built-in Tools
│   ├── File System Tools
│   ├── Search Tools
│   └── Command Execution
└── Security Layer
    ├── Path Validation
    ├── Permission Checks
    └── Sandbox Enforcement
```

**Tool Interface**:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    async fn execute(&self, args: Value) -> Result<String>;
}
```

### 5. Task Management (`crates/bedrock-task`)

**Responsibility**: Task queue and execution management

**Architecture**:
```
TaskManager
├── Priority Queue
├── Task Storage
├── Execution Engine
└── Result Cache
```

**Task Lifecycle**:
```
Created → Queued → Running → [Tool Execution] → Completed
                     ↓
                  Failed/Cancelled
```

### 6. Configuration System (`crates/bedrock-config`)

**Responsibility**: Configuration management and validation

**Features**:
- YAML parsing with serde
- Environment variable substitution
- Schema validation
- Default values
- Multi-source loading

**Configuration Flow**:
```
Load YAML → Substitute Env Vars → Validate → Apply Defaults → Return Config
```

### 7. Metrics System (`crates/bedrock-metrics`)

**Responsibility**: Observability and cost tracking

**Metrics Collected**:
- Token usage (input/output)
- Cost calculation
- Request latency
- Tool execution time
- Error rates

**Architecture**:
```
MetricsCollector
├── Token Counter
├── Cost Calculator
├── Latency Tracker
└── Export Manager
```

### 8. MCP Integration (`crates/bedrock-mcp`) ✅

**Status**: Fully functional and tested

**Architecture**:
```
MCPManager
├── Server Manager (lifecycle management)
├── Transport Layer (Stdio/SSE - both working)
├── Protocol Handler (JSON-RPC 2.0)
├── Tool Adapter (seamless integration)
└── Health Monitor (automatic recovery)
```

**Verified Integrations**:
- **Stdio**: FIGMA tools working
- **SSE**: JIRA via Redux HTTP API working
- Tool discovery and execution confirmed
- Automatic tool registration with main registry

## Data Flow Architecture

### 1. Request Processing Flow

```
User Input
    ↓
CLI Parsing
    ↓
Task Creation (UUID assigned)
    ↓
Agent Orchestration
    ↓
Bedrock Conversation
    ↓
Response Processing
    ├── Text Response → Format & Display
    └── Tool Calls → Execute → Continue
```

### 2. Tool Execution Flow

```
Tool Request from LLM
    ↓
Parse Tool Call
    ↓
Registry Lookup
    ↓
Permission Check
    ↓
Validate Arguments
    ↓
Execute in Sandbox
    ↓
Format Results
    ↓
Return to LLM
```

### 3. Streaming Response Flow

```
Bedrock Stream
    ↓
Event Parser
    ↓
Content Accumulator
    ├── Text → Display
    ├── Tool Use → Queue
    └── Metadata → Store
    ↓
Tool Execution (if needed)
    ↓
Continue Stream
```

## Security Architecture

### Layered Security Model

```
┌─────────────────────────────────────┐
│      Input Validation Layer         │
├─────────────────────────────────────┤
│      Permission Control Layer       │
├─────────────────────────────────────┤
│      Sandbox Execution Layer        │
├─────────────────────────────────────┤
│      Path Validation Layer          │
├─────────────────────────────────────┤
│      Resource Limits Layer          │
└─────────────────────────────────────┘
```

### Security Components

1. **Input Validation**
   - Schema validation for tool arguments
   - Path canonicalization
   - Command injection prevention

2. **Permission System**
   - Tool allowlist/denylist
   - Per-tool constraints
   - Dynamic permission checks

3. **Sandbox Enforcement**
   - Workspace directory isolation
   - Path traversal prevention
   - File size limits

4. **Resource Protection**
   - Execution timeouts
   - Memory limits
   - Concurrent operation limits

## Concurrency Model

### Thread Safety

```
Shared State Management
├── Arc<RwLock<ToolRegistry>>  (Multiple readers, exclusive writer)
├── Arc<Mutex<TaskQueue>>       (Exclusive access for modifications)
├── Arc<MetricsCollector>       (Thread-safe internal design)
└── Channel<Response>           (Message passing for streaming)
```

### Async Architecture

- **Runtime**: Tokio async runtime
- **Concurrency**: Multiple tools can execute in parallel
- **Streaming**: Non-blocking response processing
- **I/O**: All file and network operations are async

```rust
// Concurrent tool execution
let futures = tool_calls.iter().map(|call| {
    execute_tool(call)
});
let results = futures::future::join_all(futures).await;
```

## Error Handling Architecture

### Error Hierarchy

```
BedrockError (Root)
├── ConfigError
│   ├── ParseError
│   └── ValidationError
├── AwsError
│   ├── CredentialError
│   └── ApiError
├── ToolError
│   ├── NotFound
│   ├── ExecutionFailed
│   └── PermissionDenied
├── TaskError
│   ├── QueueFull
│   └── Timeout
└── IoError
```

### Error Propagation

```rust
// Error context preservation
Tool::execute()
    → with_context("tool_name")
    → Agent::process()
    → with_context("task_id")
    → CLI::handle()
    → Display to user
```

## Performance Architecture

### Optimization Strategies

1. **Lazy Loading**
   - Tools loaded on first use
   - Configuration cached after parsing
   - AWS client initialized once

2. **Resource Pooling**
   - Reused AWS client connections
   - Tool registry shared across requests
   - Cached credential provider

3. **Streaming Processing**
   - Incremental response display
   - Chunked file operations
   - Progressive tool execution

### Performance Metrics

```
Request Latency Budget:
├── CLI Processing: <10ms
├── Task Queue: <5ms
├── Tool Execution: Variable (timeout: 30s)
├── Bedrock API: 1-5s (model dependent)
└── Total Target: <10s for typical requests
```

## Extensibility Architecture

### Plugin System (Tools)

```rust
// Adding custom tools
impl Tool for CustomTool {
    // Implementation
}

registry.register_tool("custom", Arc::new(CustomTool::new()));
```

### Extension Points

1. **Tools**: Implement `Tool` trait
2. **Transports**: Implement `Transport` trait (MCP)
3. **Metrics**: Implement `MetricsExporter` trait
4. **Storage**: Implement `Storage` trait (future)

## Deployment Architecture

### Binary Structure

```
bedrock-agent (single binary)
├── Embedded configuration schema
├── Built-in tools
├── AWS SDK runtime
└── Async runtime
```

### Runtime Dependencies

- **Required**: AWS credentials, network access to AWS
- **Optional**: Ripgrep binary, MCP servers
- **Configuration**: YAML file or environment variables

### Container Architecture (Optional)

```dockerfile
FROM rust:slim
├── Binary installation
├── Configuration mounting
├── Workspace volume
└── Metrics export
```

## Future Architecture Considerations

### Planned Enhancements

1. **Caching Layer**
   ```
   CacheManager
   ├── LRU Cache
   ├── Persistent Storage
   ├── Invalidation Strategy
   └── Compression
   ```

2. **Rate Limiting**
   ```
   RateLimiter
   ├── Token Bucket
   ├── Per-Model Limits
   ├── Queue Management
   └── Backpressure
   ```

3. **Distributed Execution**
   ```
   Distributed Mode
   ├── Task Distribution
   ├── Result Aggregation
   ├── State Synchronization
   └── Failure Recovery
   ```

### Scalability Considerations

- **Horizontal Scaling**: Stateless design allows multiple instances
- **Vertical Scaling**: Async architecture efficiently uses resources
- **Storage Scaling**: External storage for large workspaces
- **API Scaling**: Connection pooling for high throughput

## Architecture Decisions Record (ADR)

### ADR-001: Modular Crate Structure
**Decision**: Separate functionality into independent crates
**Rationale**: Improves maintainability, enables selective compilation
**Consequences**: Clear boundaries, potential for code reuse

### ADR-002: Async-First Design
**Decision**: Use Tokio and async/await throughout
**Rationale**: Better resource utilization, non-blocking I/O
**Consequences**: Improved performance, increased complexity

### ADR-003: Tool Trait Abstraction
**Decision**: Define tools through a common trait
**Rationale**: Extensibility, consistent interface
**Consequences**: Easy to add tools, uniform error handling

### ADR-004: Streaming Response Support
**Decision**: Implement full streaming for LLM responses
**Rationale**: Better user experience, reduced latency perception
**Consequences**: Complex implementation, real-time tool execution

## Testing Architecture

### Test Pyramid

```
        ╱╲
       ╱E2E╲      (5%)  - Full system tests
      ╱──────╲
     ╱Integration╲  (25%) - Component integration
    ╱──────────────╲
   ╱   Unit Tests   ╲ (70%) - Individual functions
  ╱──────────────────╲
```

### Test Strategy

- **Unit Tests**: Each crate has comprehensive unit tests
- **Integration Tests**: Cross-crate functionality testing
- **E2E Tests**: Full workflow validation
- **Performance Tests**: Latency and throughput benchmarks

## Monitoring Architecture

### Observability Stack

```
Application
    ↓
Metrics Collector
    ├── Logs (structured JSON)
    ├── Metrics (token, cost, latency)
    └── Traces (request flow)
    ↓
Export Layer
    ├── File Export
    ├── CloudWatch (AWS)
    └── Prometheus (future)
```

## Conclusion

The Bedrock CLI Agent architecture prioritizes:
1. **Modularity** - Clear separation of concerns
2. **Security** - Multiple layers of protection
3. **Performance** - Async operations and streaming
4. **Extensibility** - Plugin-based tool system
5. **Observability** - Comprehensive metrics and logging

This architecture provides a solid foundation for a production-ready CLI agent while maintaining flexibility for future enhancements.

---

*For implementation details, see the [API Documentation](../api/crates/). For deployment, see the [Getting Started Guide](../guides/getting-started.md).*
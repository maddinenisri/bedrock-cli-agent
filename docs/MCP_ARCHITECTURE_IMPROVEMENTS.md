# MCP Architecture Improvements: Clean Architecture Design

## Executive Summary

This document outlines architectural improvements for the Model Context Protocol (MCP) integration within the Bedrock CLI Agent system. The proposed architecture addresses separation of concerns, clean abstraction layers, efficient message routing, and improved error handling while maintaining integration with AWS Bedrock runtime.

## Current Architecture Analysis

### Identified Issues

1. **Tight Coupling**: Manager directly handles transport creation and client lifecycle
2. **Mixed Responsibilities**: Client handles both protocol logic and transport concerns  
3. **Limited Error Recovery**: Basic retry logic without circuit breaker patterns
4. **Tool Registry Integration**: Direct coupling between MCP tools and Bedrock tool registry
5. **Configuration Management**: Static configuration loading without hot-reload
6. **Observability Gaps**: Limited metrics and distributed tracing

## Improved Architecture Design

### Core Principles

1. **Separation of Concerns**: Clear boundaries between protocol, transport, and business logic
2. **Dependency Inversion**: Abstract interfaces with concrete implementations
3. **Single Responsibility**: Each component has one clear purpose
4. **Open/Closed Principle**: Extensible without modification
5. **Resilience**: Built-in error recovery and fault tolerance

### Architecture Layers

```
┌─────────────────────────────────────────────────────┐
│                Application Layer                    │
├─────────────────────────────────────────────────────┤
│              MCP Service Interface                  │
├─────────────────────────────────────────────────────┤
│                 Protocol Layer                      │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │   Message   │  │   Session    │  │    Tool     │ │
│  │   Router    │  │   Manager    │  │  Discovery  │ │
│  └─────────────┘  └──────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────┤
│                Transport Layer                      │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ Connection  │  │   Message    │  │   Health    │ │
│  │    Pool     │  │  Dispatcher  │  │   Monitor   │ │
│  └─────────────┘  └──────────────┘  └─────────────┘ │
├─────────────────────────────────────────────────────┤
│                Infrastructure                       │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │   Config    │  │ Observability│  │   Error     │ │
│  │  Manager    │  │    Layer     │  │  Recovery   │ │
│  └─────────────┘  └──────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Component Design

### 1. MCP Service Interface

**Purpose**: Primary facade providing high-level MCP operations

```rust
#[async_trait]
pub trait McpService: Send + Sync {
    async fn discover_servers(&self) -> Result<Vec<ServerInfo>>;
    async fn connect_to_server(&self, server_id: &str) -> Result<SessionId>;
    async fn list_tools(&self, session_id: SessionId) -> Result<Vec<ToolInfo>>;
    async fn execute_tool(&self, session_id: SessionId, request: ToolRequest) -> Result<ToolResponse>;
    async fn subscribe_to_events(&self, handler: Box<dyn EventHandler>) -> Result<()>;
}
```

**Benefits**:
- Clean API for higher-level components
- Hides protocol complexity
- Consistent error handling
- Event-driven architecture support

### 2. Session Manager

**Purpose**: Manages MCP session lifecycle and state

```rust
#[async_trait]
pub trait SessionManager: Send + Sync {
    async fn create_session(&self, server_config: ServerConfig) -> Result<SessionHandle>;
    async fn get_session(&self, session_id: SessionId) -> Option<Arc<Session>>;
    async fn close_session(&self, session_id: SessionId) -> Result<()>;
    async fn health_check(&self, session_id: SessionId) -> Result<HealthStatus>;
}

pub struct Session {
    id: SessionId,
    server_info: ServerInfo,
    capabilities: ServerCapabilities,
    connection: Arc<dyn Connection>,
    message_router: Arc<MessageRouter>,
    tool_catalog: Arc<ToolCatalog>,
    state: Arc<RwLock<SessionState>>,
}
```

**Benefits**:
- Centralized session lifecycle management
- State isolation between sessions
- Connection pooling and reuse
- Automatic health monitoring

### 3. Message Router

**Purpose**: Routes messages between protocol and transport layers

```rust
#[async_trait] 
pub trait MessageRouter: Send + Sync {
    async fn send_request(&self, request: Request) -> Result<Response>;
    async fn send_notification(&self, notification: Notification) -> Result<()>;
    async fn handle_incoming(&self, message: IncomingMessage) -> Result<()>;
    async fn register_handler(&self, method: &str, handler: Box<dyn MessageHandler>) -> Result<()>;
}

pub struct RoutingTable {
    request_handlers: HashMap<String, Box<dyn RequestHandler>>,
    notification_handlers: HashMap<String, Box<dyn NotificationHandler>>,
    middleware: Vec<Box<dyn MessageMiddleware>>,
}
```

**Benefits**:
- Decoupled message handling
- Middleware support for cross-cutting concerns
- Type-safe routing
- Request correlation and timeout handling

### 4. Connection Pool

**Purpose**: Manages transport connections with pooling and load balancing

```rust
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    async fn acquire(&self, server_id: &str) -> Result<PooledConnection>;
    async fn release(&self, connection: PooledConnection) -> Result<()>;
    async fn health_check_all(&self) -> Vec<ConnectionHealth>;
    async fn drain(&self, server_id: &str) -> Result<()>;
}

pub struct PoolConfiguration {
    pub min_connections: usize,
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub health_check_interval: Duration,
}
```

**Benefits**:
- Connection reuse and efficiency
- Load balancing across multiple connections
- Automatic connection health monitoring
- Graceful connection draining

### 5. Tool Discovery Service

**Purpose**: Discovers, validates, and registers MCP tools

```rust
#[async_trait]
pub trait ToolDiscoveryService: Send + Sync {
    async fn discover_tools(&self, session_id: SessionId) -> Result<Vec<ToolDefinition>>;
    async fn validate_tool(&self, tool: &ToolDefinition) -> Result<ValidationResult>;
    async fn register_tool(&self, tool: ToolDefinition, session_id: SessionId) -> Result<()>;
    async fn unregister_tools(&self, session_id: SessionId) -> Result<()>;
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub schema: JsonSchema,
    pub server_id: String,
    pub session_id: SessionId,
    pub metadata: ToolMetadata,
}
```

**Benefits**:
- Automated tool discovery and registration
- Schema validation and compatibility checking
- Conflict resolution for tool names
- Metadata-driven tool management

### 6. Error Recovery System

**Purpose**: Implements resilience patterns for fault tolerance

```rust
pub struct ErrorRecoveryPolicy {
    pub retry_policy: RetryPolicy,
    pub circuit_breaker: CircuitBreakerConfig,
    pub fallback_strategy: FallbackStrategy,
    pub timeout_policy: TimeoutPolicy,
}

#[async_trait]
pub trait ErrorRecoveryService: Send + Sync {
    async fn execute_with_recovery<F, T>(&self, operation: F, policy: &ErrorRecoveryPolicy) -> Result<T>
    where
        F: Future<Output = Result<T>> + Send;
    
    async fn handle_connection_failure(&self, session_id: SessionId, error: &BedrockError) -> RecoveryAction;
    async fn trigger_circuit_breaker(&self, session_id: SessionId, reason: &str) -> Result<()>;
}
```

**Benefits**:
- Automatic retry with exponential backoff
- Circuit breaker protection
- Graceful degradation strategies
- Connection failure recovery

### 7. Configuration Management

**Purpose**: Dynamic configuration with hot-reload support

```rust
#[async_trait]
pub trait ConfigurationManager: Send + Sync {
    async fn load_configuration(&self, sources: Vec<ConfigSource>) -> Result<McpConfiguration>;
    async fn reload_configuration(&self) -> Result<()>;
    async fn watch_for_changes(&self, callback: Box<dyn ConfigChangeHandler>) -> Result<()>;
    async fn validate_configuration(&self, config: &McpConfiguration) -> Result<ValidationReport>;
}

pub struct McpConfiguration {
    pub servers: HashMap<String, ServerConfiguration>,
    pub transport: TransportConfiguration,
    pub security: SecurityConfiguration,
    pub observability: ObservabilityConfiguration,
}
```

**Benefits**:
- Hot configuration reloading
- Multiple configuration sources
- Configuration validation
- Change event notifications

### 8. Observability Layer

**Purpose**: Comprehensive metrics, tracing, and logging

```rust
#[async_trait]
pub trait ObservabilityService: Send + Sync {
    async fn record_metric(&self, metric: Metric) -> Result<()>;
    async fn start_trace(&self, operation: &str) -> Result<TraceId>;
    async fn end_trace(&self, trace_id: TraceId, result: &Result<()>) -> Result<()>;
    async fn log_event(&self, event: LogEvent) -> Result<()>;
}

pub struct McpMetrics {
    pub connection_count: Gauge,
    pub request_latency: Histogram,
    pub error_rate: Counter,
    pub tool_execution_time: Histogram,
}
```

**Benefits**:
- Distributed tracing across MCP calls
- Comprehensive metrics collection
- Structured logging with correlation
- Performance monitoring and alerting

## Integration Points

### 1. Bedrock Tool Registry Integration

```rust
pub struct McpToolAdapter {
    tool_definition: ToolDefinition,
    session_manager: Arc<dyn SessionManager>,
    error_recovery: Arc<dyn ErrorRecoveryService>,
    metrics: Arc<McpMetrics>,
}

#[async_trait]
impl Tool for McpToolAdapter {
    async fn execute(&self, args: Value) -> Result<Value> {
        let session_id = self.resolve_session().await?;
        
        self.error_recovery.execute_with_recovery(
            || self.execute_tool_internal(session_id, args.clone()),
            &self.get_recovery_policy(),
        ).await
    }
}
```

### 2. Agent Integration

```rust
impl Agent {
    async fn initialize_mcp(&mut self) -> Result<()> {
        let mcp_service = McpServiceBuilder::new()
            .with_configuration_manager(self.config_manager.clone())
            .with_session_manager(DefaultSessionManager::new())
            .with_connection_pool(DefaultConnectionPool::new())
            .with_error_recovery(ErrorRecoveryService::new())
            .with_observability(self.observability.clone())
            .build()?;
        
        // Discover and register tools
        let servers = mcp_service.discover_servers().await?;
        for server in servers {
            let session_id = mcp_service.connect_to_server(&server.id).await?;
            let tools = mcp_service.list_tools(session_id).await?;
            
            for tool in tools {
                let adapter = McpToolAdapter::new(tool, mcp_service.clone());
                self.tool_registry.register(adapter)?;
            }
        }
        
        self.mcp_service = Some(mcp_service);
        Ok(())
    }
}
```

## Migration Strategy

### Phase 1: Infrastructure Layer
1. Implement configuration management system
2. Add observability infrastructure
3. Create error recovery framework
4. Set up connection pooling

### Phase 2: Protocol Layer
1. Implement message router
2. Create session manager
3. Add tool discovery service
4. Integrate with existing transport layer

### Phase 3: Service Layer
1. Implement MCP service facade
2. Create tool adapters
3. Update agent integration
4. Add comprehensive testing

### Phase 4: Optimization
1. Performance tuning
2. Advanced resilience patterns
3. Monitoring and alerting
4. Documentation and training

## Benefits of Improved Architecture

### Technical Benefits
- **Maintainability**: Clear separation of concerns and modular design
- **Scalability**: Connection pooling and efficient resource management
- **Reliability**: Circuit breakers, retries, and error recovery
- **Observability**: Comprehensive monitoring and debugging capabilities
- **Extensibility**: Plugin architecture for new transports and protocols

### Operational Benefits
- **Configuration Management**: Hot-reload without service restart
- **Monitoring**: Real-time health and performance metrics
- **Debugging**: Distributed tracing and structured logging
- **Deployment**: Rolling updates and blue-green deployments
- **Testing**: Isolated unit testing and integration testing

### Business Benefits
- **Reduced Downtime**: Automatic error recovery and failover
- **Faster Development**: Clean interfaces and comprehensive testing
- **Better User Experience**: Consistent performance and reliability
- **Cost Optimization**: Efficient resource utilization
- **Risk Mitigation**: Comprehensive error handling and monitoring

## Conclusion

The proposed architecture improvements provide a solid foundation for scalable, maintainable, and reliable MCP integration. The clean separation of concerns, comprehensive error handling, and observability features will significantly improve the system's operational characteristics while maintaining compatibility with existing Bedrock components.

The migration can be performed incrementally, allowing for continuous operation during the transition period. The investment in architectural improvements will pay dividends in reduced maintenance overhead, improved reliability, and faster feature development.
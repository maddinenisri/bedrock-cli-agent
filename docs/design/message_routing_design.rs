// Message Routing and Dispatch System Design
// This file contains the detailed design for efficient message routing and dispatch

use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
    fmt::Debug,
};
use tokio::sync::{RwLock, mpsc, oneshot};
use uuid::Uuid;
use serde_json::Value;
use bedrock_core::{Result, BedrockError};

/// Request correlation ID for tracking
pub type CorrelationId = Uuid;

/// Message types that can be routed
#[derive(Debug, Clone)]
pub enum RoutableMessage {
    Request {
        id: CorrelationId,
        method: String,
        params: Option<Value>,
        timeout: Duration,
        response_channel: oneshot::Sender<Result<Value>>,
    },
    Response {
        id: CorrelationId,
        result: Option<Value>,
        error: Option<Value>,
    },
    Notification {
        method: String,
        params: Option<Value>,
    },
    Event {
        event_type: String,
        payload: Value,
        source: String,
    },
}

/// Request context for middleware
#[derive(Debug)]
pub struct RequestContext {
    pub correlation_id: CorrelationId,
    pub method: String,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub started_at: Instant,
    pub metadata: HashMap<String, Value>,
}

/// Response context for middleware
#[derive(Debug)]
pub struct ResponseContext {
    pub correlation_id: CorrelationId,
    pub method: String,
    pub duration: Duration,
    pub success: bool,
    pub error_code: Option<String>,
}

/// Message handler trait for requests
#[async_trait]
pub trait RequestHandler: Send + Sync + Debug {
    async fn handle_request(
        &self,
        method: &str,
        params: Option<Value>,
        context: RequestContext,
    ) -> Result<Value>;
    
    fn supported_methods(&self) -> Vec<String>;
    fn handler_name(&self) -> &str;
}

/// Message handler trait for notifications
#[async_trait]
pub trait NotificationHandler: Send + Sync + Debug {
    async fn handle_notification(
        &self,
        method: &str,
        params: Option<Value>,
        context: RequestContext,
    ) -> Result<()>;
    
    fn supported_methods(&self) -> Vec<String>;
    fn handler_name(&self) -> &str;
}

/// Event handler trait for system events
#[async_trait]
pub trait EventHandler: Send + Sync + Debug {
    async fn handle_event(
        &self,
        event_type: &str,
        payload: Value,
        context: EventContext,
    ) -> Result<()>;
    
    fn supported_events(&self) -> Vec<String>;
    fn handler_name(&self) -> &str;
}

/// Event context
#[derive(Debug)]
pub struct EventContext {
    pub event_id: Uuid,
    pub source: String,
    pub timestamp: Instant,
    pub metadata: HashMap<String, Value>,
}

/// Middleware for cross-cutting concerns
#[async_trait]
pub trait MessageMiddleware: Send + Sync + Debug {
    /// Called before request processing
    async fn pre_request(
        &self,
        context: &mut RequestContext,
    ) -> Result<()>;
    
    /// Called after successful request processing
    async fn post_request(
        &self,
        context: &RequestContext,
        response: &Value,
    ) -> Result<()>;
    
    /// Called when request processing fails
    async fn on_error(
        &self,
        context: &RequestContext,
        error: &BedrockError,
    ) -> Result<()>;
    
    fn middleware_name(&self) -> &str;
    fn priority(&self) -> i32; // Higher values execute first
}

/// Message router configuration
#[derive(Debug, Clone)]
pub struct RouterConfiguration {
    /// Maximum number of concurrent requests per session
    pub max_concurrent_requests: usize,
    /// Request timeout duration
    pub default_request_timeout: Duration,
    /// Maximum request queue size
    pub max_queue_size: usize,
    /// Enable request/response correlation tracking
    pub enable_correlation_tracking: bool,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
}

impl Default for RouterConfiguration {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 100,
            default_request_timeout: Duration::from_secs(30),
            max_queue_size: 1000,
            enable_correlation_tracking: true,
            enable_metrics: true,
        }
    }
}

/// Core message router trait
#[async_trait]
pub trait MessageRouter: Send + Sync {
    /// Send a request and wait for response
    async fn send_request(
        &self,
        method: String,
        params: Option<Value>,
        timeout: Option<Duration>,
    ) -> Result<Value>;
    
    /// Send a notification (fire-and-forget)
    async fn send_notification(
        &self,
        method: String,
        params: Option<Value>,
    ) -> Result<()>;
    
    /// Publish an event to subscribers
    async fn publish_event(
        &self,
        event_type: String,
        payload: Value,
        source: String,
    ) -> Result<()>;
    
    /// Register a request handler
    async fn register_request_handler(
        &self,
        handler: Arc<dyn RequestHandler>,
    ) -> Result<()>;
    
    /// Register a notification handler
    async fn register_notification_handler(
        &self,
        handler: Arc<dyn NotificationHandler>,
    ) -> Result<()>;
    
    /// Register an event handler
    async fn register_event_handler(
        &self,
        handler: Arc<dyn EventHandler>,
    ) -> Result<()>;
    
    /// Add middleware to the processing pipeline
    async fn add_middleware(
        &self,
        middleware: Arc<dyn MessageMiddleware>,
    ) -> Result<()>;
    
    /// Get router statistics
    async fn get_statistics(&self) -> RouterStatistics;
    
    /// Shutdown the router
    async fn shutdown(&self) -> Result<()>;
}

/// Router statistics for monitoring
#[derive(Debug)]
pub struct RouterStatistics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_notifications: u64,
    pub total_events: u64,
    pub active_requests: usize,
    pub average_response_time_ms: u64,
    pub queue_size: usize,
    pub registered_handlers: usize,
    pub registered_middleware: usize,
}

/// Routing table for efficient handler lookup
struct RoutingTable {
    request_handlers: HashMap<String, Arc<dyn RequestHandler>>,
    notification_handlers: HashMap<String, Vec<Arc<dyn NotificationHandler>>>,
    event_handlers: HashMap<String, Vec<Arc<dyn EventHandler>>>,
    middleware: Vec<Arc<dyn MessageMiddleware>>,
}

impl RoutingTable {
    fn new() -> Self {
        Self {
            request_handlers: HashMap::new(),
            notification_handlers: HashMap::new(),
            event_handlers: HashMap::new(),
            middleware: Vec::new(),
        }
    }
    
    fn add_request_handler(&mut self, handler: Arc<dyn RequestHandler>) -> Result<()> {
        for method in handler.supported_methods() {
            if self.request_handlers.contains_key(&method) {
                return Err(BedrockError::ConfigError(
                    format!("Request handler for method '{}' already registered", method)
                ));
            }
            self.request_handlers.insert(method, handler.clone());
        }
        Ok(())
    }
    
    fn add_notification_handler(&mut self, handler: Arc<dyn NotificationHandler>) {
        for method in handler.supported_methods() {
            self.notification_handlers
                .entry(method)
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }
    }
    
    fn add_event_handler(&mut self, handler: Arc<dyn EventHandler>) {
        for event_type in handler.supported_events() {
            self.event_handlers
                .entry(event_type)
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }
    }
    
    fn add_middleware(&mut self, middleware: Arc<dyn MessageMiddleware>) {
        self.middleware.push(middleware);
        // Sort middleware by priority (higher priority first)
        self.middleware.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }
    
    fn get_request_handler(&self, method: &str) -> Option<&Arc<dyn RequestHandler>> {
        self.request_handlers.get(method)
    }
    
    fn get_notification_handlers(&self, method: &str) -> Vec<&Arc<dyn NotificationHandler>> {
        self.notification_handlers
            .get(method)
            .map(|handlers| handlers.iter().collect())
            .unwrap_or_default()
    }
    
    fn get_event_handlers(&self, event_type: &str) -> Vec<&Arc<dyn EventHandler>> {
        self.event_handlers
            .get(event_type)
            .map(|handlers| handlers.iter().collect())
            .unwrap_or_default()
    }
}

/// Main message router implementation
pub struct MessageRouterImpl {
    config: RouterConfiguration,
    routing_table: Arc<RwLock<RoutingTable>>,
    // Channel for incoming messages
    message_sender: mpsc::UnboundedSender<RoutableMessage>,
    // Pending requests tracking
    pending_requests: Arc<RwLock<HashMap<CorrelationId, PendingRequest>>>,
    // Router statistics
    statistics: Arc<RwLock<RouterStatistics>>,
    // Task handles for cleanup
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
struct PendingRequest {
    method: String,
    started_at: Instant,
    timeout: Duration,
    response_sender: oneshot::Sender<Result<Value>>,
}

impl MessageRouterImpl {
    pub fn new(config: RouterConfiguration) -> Self {
        let (message_sender, message_receiver) = mpsc::unbounded_channel();
        let routing_table = Arc::new(RwLock::new(RoutingTable::new()));
        let pending_requests = Arc::new(RwLock::new(HashMap::new()));
        let statistics = Arc::new(RwLock::new(RouterStatistics::default()));
        
        let mut router = Self {
            config,
            routing_table: routing_table.clone(),
            message_sender,
            pending_requests: pending_requests.clone(),
            statistics: statistics.clone(),
            task_handles: Vec::new(),
        };
        
        // Start message processing task
        let processing_handle = Self::start_message_processing_task(
            message_receiver,
            routing_table,
            pending_requests,
            statistics,
            router.config.clone(),
        );
        router.task_handles.push(processing_handle);
        
        // Start timeout monitoring task
        let timeout_handle = Self::start_timeout_monitoring_task(
            router.pending_requests.clone(),
            router.statistics.clone(),
        );
        router.task_handles.push(timeout_handle);
        
        router
    }
    
    fn start_message_processing_task(
        mut message_receiver: mpsc::UnboundedReceiver<RoutableMessage>,
        routing_table: Arc<RwLock<RoutingTable>>,
        pending_requests: Arc<RwLock<HashMap<CorrelationId, PendingRequest>>>,
        statistics: Arc<RwLock<RouterStatistics>>,
        config: RouterConfiguration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(message) = message_receiver.recv().await {
                let table = routing_table.read().await;
                let result = Self::process_message(
                    message,
                    &table,
                    &pending_requests,
                    &statistics,
                    &config,
                ).await;
                
                if let Err(e) = result {
                    tracing::error!("Error processing message: {}", e);
                }
            }
        })
    }
    
    fn start_timeout_monitoring_task(
        pending_requests: Arc<RwLock<HashMap<CorrelationId, PendingRequest>>>,
        statistics: Arc<RwLock<RouterStatistics>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let mut requests = pending_requests.write().await;
                let mut timed_out_requests = Vec::new();
                
                // Find timed out requests
                for (correlation_id, pending_request) in requests.iter() {
                    if now.duration_since(pending_request.started_at) > pending_request.timeout {
                        timed_out_requests.push(*correlation_id);
                    }
                }
                
                // Remove timed out requests and notify senders
                for correlation_id in timed_out_requests {
                    if let Some(pending_request) = requests.remove(&correlation_id) {
                        let _ = pending_request.response_sender.send(
                            Err(BedrockError::McpError(
                                format!("Request {} timed out", correlation_id)
                            ))
                        );
                        
                        // Update statistics
                        let mut stats = statistics.write().await;
                        stats.failed_requests += 1;
                    }
                }
            }
        })
    }
    
    async fn process_message(
        message: RoutableMessage,
        routing_table: &RoutingTable,
        pending_requests: &Arc<RwLock<HashMap<CorrelationId, PendingRequest>>>,
        statistics: &Arc<RwLock<RouterStatistics>>,
        _config: &RouterConfiguration,
    ) -> Result<()> {
        match message {
            RoutableMessage::Request { id, method, params, timeout: _, response_channel } => {
                Self::process_request(
                    id,
                    method,
                    params,
                    response_channel,
                    routing_table,
                    statistics,
                ).await
            }
            RoutableMessage::Response { id, result, error } => {
                Self::process_response(id, result, error, pending_requests, statistics).await
            }
            RoutableMessage::Notification { method, params } => {
                Self::process_notification(method, params, routing_table, statistics).await
            }
            RoutableMessage::Event { event_type, payload, source } => {
                Self::process_event(event_type, payload, source, routing_table, statistics).await
            }
        }
    }
    
    async fn process_request(
        correlation_id: CorrelationId,
        method: String,
        params: Option<Value>,
        response_channel: oneshot::Sender<Result<Value>>,
        routing_table: &RoutingTable,
        statistics: &Arc<RwLock<RouterStatistics>>,
    ) -> Result<()> {
        let start_time = Instant::now();
        
        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.total_requests += 1;
            stats.active_requests += 1;
        }
        
        // Create request context
        let mut context = RequestContext {
            correlation_id,
            method: method.clone(),
            session_id: None,
            user_id: None,
            started_at: start_time,
            metadata: HashMap::new(),
        };
        
        // Execute middleware pre-request phase
        for middleware in &routing_table.middleware {
            if let Err(e) = middleware.pre_request(&mut context).await {
                tracing::error!("Middleware {} pre-request failed: {}", middleware.middleware_name(), e);
                let _ = response_channel.send(Err(e));
                return Ok(());
            }
        }
        
        // Find and execute handler
        let result = if let Some(handler) = routing_table.get_request_handler(&method) {
            handler.handle_request(&method, params, context).await
        } else {
            Err(BedrockError::McpError(format!("No handler found for method: {}", method)))
        };
        
        // Execute middleware post-request phase
        let request_context = RequestContext {
            correlation_id,
            method: method.clone(),
            session_id: None,
            user_id: None,
            started_at: start_time,
            metadata: HashMap::new(),
        };
        
        match &result {
            Ok(response) => {
                for middleware in &routing_table.middleware {
                    if let Err(e) = middleware.post_request(&request_context, response).await {
                        tracing::error!("Middleware {} post-request failed: {}", middleware.middleware_name(), e);
                    }
                }
            }
            Err(error) => {
                for middleware in &routing_table.middleware {
                    if let Err(e) = middleware.on_error(&request_context, error).await {
                        tracing::error!("Middleware {} on-error failed: {}", middleware.middleware_name(), e);
                    }
                }
            }
        }
        
        // Send response
        let _ = response_channel.send(result);
        
        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.active_requests = stats.active_requests.saturating_sub(1);
            if result.is_ok() {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }
        }
        
        Ok(())
    }
    
    async fn process_response(
        correlation_id: CorrelationId,
        result: Option<Value>,
        error: Option<Value>,
        pending_requests: &Arc<RwLock<HashMap<CorrelationId, PendingRequest>>>,
        statistics: &Arc<RwLock<RouterStatistics>>,
    ) -> Result<()> {
        let mut requests = pending_requests.write().await;
        if let Some(pending_request) = requests.remove(&correlation_id) {
            let response_result = if let Some(error) = error {
                Err(BedrockError::McpError(format!("Remote error: {}", error)))
            } else {
                Ok(result.unwrap_or(Value::Null))
            };
            
            let _ = pending_request.response_sender.send(response_result);
            
            // Update statistics
            let mut stats = statistics.write().await;
            stats.active_requests = stats.active_requests.saturating_sub(1);
            if error.is_none() {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }
        }
        
        Ok(())
    }
    
    async fn process_notification(
        method: String,
        params: Option<Value>,
        routing_table: &RoutingTable,
        statistics: &Arc<RwLock<RouterStatistics>>,
    ) -> Result<()> {
        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.total_notifications += 1;
        }
        
        let context = RequestContext {
            correlation_id: Uuid::new_v4(),
            method: method.clone(),
            session_id: None,
            user_id: None,
            started_at: Instant::now(),
            metadata: HashMap::new(),
        };
        
        let handlers = routing_table.get_notification_handlers(&method);
        for handler in handlers {
            if let Err(e) = handler.handle_notification(&method, params.clone(), context).await {
                tracing::error!("Notification handler {} failed: {}", handler.handler_name(), e);
            }
        }
        
        Ok(())
    }
    
    async fn process_event(
        event_type: String,
        payload: Value,
        source: String,
        routing_table: &RoutingTable,
        statistics: &Arc<RwLock<RouterStatistics>>,
    ) -> Result<()> {
        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.total_events += 1;
        }
        
        let context = EventContext {
            event_id: Uuid::new_v4(),
            source,
            timestamp: Instant::now(),
            metadata: HashMap::new(),
        };
        
        let handlers = routing_table.get_event_handlers(&event_type);
        for handler in handlers {
            if let Err(e) = handler.handle_event(&event_type, payload.clone(), context).await {
                tracing::error!("Event handler {} failed: {}", handler.handler_name(), e);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl MessageRouter for MessageRouterImpl {
    async fn send_request(
        &self,
        method: String,
        params: Option<Value>,
        timeout: Option<Duration>,
    ) -> Result<Value> {
        let correlation_id = Uuid::new_v4();
        let timeout = timeout.unwrap_or(self.config.default_request_timeout);
        let (response_sender, response_receiver) = oneshot::channel();
        
        // Store pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(correlation_id, PendingRequest {
                method: method.clone(),
                started_at: Instant::now(),
                timeout,
                response_sender,
            });
        }
        
        // Send request message
        let message = RoutableMessage::Request {
            id: correlation_id,
            method,
            params,
            timeout,
            response_channel: response_sender,
        };
        
        self.message_sender.send(message)
            .map_err(|_| BedrockError::McpError("Router message channel closed".into()))?;
        
        // Wait for response
        response_receiver.await
            .map_err(|_| BedrockError::McpError("Response channel closed".into()))?
    }
    
    async fn send_notification(
        &self,
        method: String,
        params: Option<Value>,
    ) -> Result<()> {
        let message = RoutableMessage::Notification { method, params };
        
        self.message_sender.send(message)
            .map_err(|_| BedrockError::McpError("Router message channel closed".into()))?;
        
        Ok(())
    }
    
    async fn publish_event(
        &self,
        event_type: String,
        payload: Value,
        source: String,
    ) -> Result<()> {
        let message = RoutableMessage::Event {
            event_type,
            payload,
            source,
        };
        
        self.message_sender.send(message)
            .map_err(|_| BedrockError::McpError("Router message channel closed".into()))?;
        
        Ok(())
    }
    
    async fn register_request_handler(
        &self,
        handler: Arc<dyn RequestHandler>,
    ) -> Result<()> {
        let mut table = self.routing_table.write().await;
        table.add_request_handler(handler)
    }
    
    async fn register_notification_handler(
        &self,
        handler: Arc<dyn NotificationHandler>,
    ) -> Result<()> {
        let mut table = self.routing_table.write().await;
        table.add_notification_handler(handler);
        Ok(())
    }
    
    async fn register_event_handler(
        &self,
        handler: Arc<dyn EventHandler>,
    ) -> Result<()> {
        let mut table = self.routing_table.write().await;
        table.add_event_handler(handler);
        Ok(())
    }
    
    async fn add_middleware(
        &self,
        middleware: Arc<dyn MessageMiddleware>,
    ) -> Result<()> {
        let mut table = self.routing_table.write().await;
        table.add_middleware(middleware);
        Ok(())
    }
    
    async fn get_statistics(&self) -> RouterStatistics {
        let stats = self.statistics.read().await;
        RouterStatistics {
            total_requests: stats.total_requests,
            successful_requests: stats.successful_requests,
            failed_requests: stats.failed_requests,
            total_notifications: stats.total_notifications,
            total_events: stats.total_events,
            active_requests: stats.active_requests,
            average_response_time_ms: stats.average_response_time_ms,
            queue_size: stats.queue_size,
            registered_handlers: stats.registered_handlers,
            registered_middleware: stats.registered_middleware,
        }
    }
    
    async fn shutdown(&self) -> Result<()> {
        // Cancel all task handles
        for handle in &self.task_handles {
            handle.abort();
        }
        
        // Clear pending requests
        let mut pending = self.pending_requests.write().await;
        for (_, pending_request) in pending.drain() {
            let _ = pending_request.response_sender.send(
                Err(BedrockError::McpError("Router shutting down".into()))
            );
        }
        
        Ok(())
    }
}

impl Default for RouterStatistics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_notifications: 0,
            total_events: 0,
            active_requests: 0,
            average_response_time_ms: 0,
            queue_size: 0,
            registered_handlers: 0,
            registered_middleware: 0,
        }
    }
}

// Example middleware implementations
#[derive(Debug)]
pub struct LoggingMiddleware;

#[async_trait]
impl MessageMiddleware for LoggingMiddleware {
    async fn pre_request(&self, context: &mut RequestContext) -> Result<()> {
        tracing::info!(
            "Request started: {} [{}]",
            context.method,
            context.correlation_id
        );
        Ok(())
    }
    
    async fn post_request(&self, context: &RequestContext, _response: &Value) -> Result<()> {
        let duration = context.started_at.elapsed();
        tracing::info!(
            "Request completed: {} [{}] in {:?}",
            context.method,
            context.correlation_id,
            duration
        );
        Ok(())
    }
    
    async fn on_error(&self, context: &RequestContext, error: &BedrockError) -> Result<()> {
        let duration = context.started_at.elapsed();
        tracing::error!(
            "Request failed: {} [{}] after {:?}: {}",
            context.method,
            context.correlation_id,
            duration,
            error
        );
        Ok(())
    }
    
    fn middleware_name(&self) -> &str {
        "LoggingMiddleware"
    }
    
    fn priority(&self) -> i32 {
        100 // High priority to log all requests
    }
}

#[derive(Debug)]
pub struct MetricsMiddleware {
    // Would contain metrics collectors
}

#[async_trait]
impl MessageMiddleware for MetricsMiddleware {
    async fn pre_request(&self, _context: &mut RequestContext) -> Result<()> {
        // Record request start metric
        Ok(())
    }
    
    async fn post_request(&self, context: &RequestContext, _response: &Value) -> Result<()> {
        // Record successful request metric
        let duration = context.started_at.elapsed();
        tracing::debug!("Request {} took {:?}", context.method, duration);
        Ok(())
    }
    
    async fn on_error(&self, context: &RequestContext, _error: &BedrockError) -> Result<()> {
        // Record failed request metric
        let duration = context.started_at.elapsed();
        tracing::debug!("Failed request {} took {:?}", context.method, duration);
        Ok(())
    }
    
    fn middleware_name(&self) -> &str {
        "MetricsMiddleware"
    }
    
    fn priority(&self) -> i32 {
        50 // Medium priority for metrics
    }
}
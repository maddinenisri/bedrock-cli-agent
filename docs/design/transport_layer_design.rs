// Transport Layer Design - Connection Pool and Message Dispatcher
// This file contains the detailed design for the improved transport layer

use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{RwLock, Semaphore, mpsc};
use uuid::Uuid;
use bedrock_core::{Result, BedrockError};

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfiguration {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Timeout for acquiring a connection
    pub acquire_timeout: Duration,
    /// Maximum idle time before connection is closed
    pub idle_timeout: Duration,
    /// Interval for health check probes
    pub health_check_interval: Duration,
    /// Maximum number of retry attempts
    pub max_retries: usize,
}

impl Default for PoolConfiguration {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(60),
            max_retries: 3,
        }
    }
}

/// Connection health status
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    pub connection_id: Uuid,
    pub server_id: String,
    pub is_healthy: bool,
    pub last_used: Instant,
    pub last_health_check: Instant,
    pub error_count: usize,
    pub latency_ms: u64,
}

/// Pooled connection wrapper
pub struct PooledConnection {
    pub id: Uuid,
    pub server_id: String,
    pub transport: Box<dyn Transport>,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: usize,
    pool: Arc<ConnectionPoolImpl>,
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // Return connection to pool when dropped
        let pool = self.pool.clone();
        let connection = std::mem::replace(&mut self.transport, Box::new(NullTransport));
        let connection_info = ConnectionInfo {
            id: self.id,
            server_id: self.server_id.clone(),
            created_at: self.created_at,
            last_used: self.last_used,
            use_count: self.use_count,
        };
        
        tokio::spawn(async move {
            if let Err(e) = pool.return_connection(connection, connection_info).await {
                tracing::warn!("Failed to return connection to pool: {}", e);
            }
        });
    }
}

/// Connection pool trait
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    /// Acquire a connection for the given server
    async fn acquire(&self, server_id: &str) -> Result<PooledConnection>;
    
    /// Get health status for all connections
    async fn health_status(&self) -> Vec<ConnectionHealth>;
    
    /// Drain all connections for a server
    async fn drain(&self, server_id: &str) -> Result<()>;
    
    /// Get pool statistics
    async fn statistics(&self) -> PoolStatistics;
    
    /// Shutdown the pool
    async fn shutdown(&self) -> Result<()>;
}

/// Pool statistics
#[derive(Debug)]
pub struct PoolStatistics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub failed_connections: usize,
    pub total_acquisitions: u64,
    pub total_releases: u64,
    pub average_acquisition_time_ms: u64,
}

/// Connection info for tracking
#[derive(Debug, Clone)]
struct ConnectionInfo {
    id: Uuid,
    server_id: String,
    created_at: Instant,
    last_used: Instant,
    use_count: usize,
}

/// Connection pool implementation
pub struct ConnectionPoolImpl {
    config: PoolConfiguration,
    // Server ID -> Pool of connections
    pools: Arc<RwLock<HashMap<String, ServerConnectionPool>>>,
    // Semaphore for limiting total connections
    connection_semaphore: Arc<Semaphore>,
    // Health check task handle
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    // Metrics
    metrics: Arc<PoolMetrics>,
}

/// Per-server connection pool
struct ServerConnectionPool {
    server_id: String,
    config: ServerConfiguration,
    // Available connections
    available: Vec<(Box<dyn Transport>, ConnectionInfo)>,
    // Active connections count
    active_count: usize,
    // Failed connections count
    failed_count: usize,
    // Waiters for connections
    waiters: Vec<tokio::sync::oneshot::Sender<Result<PooledConnection>>>,
}

impl ConnectionPoolImpl {
    pub fn new(config: PoolConfiguration) -> Self {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));
        let metrics = Arc::new(PoolMetrics::new());
        
        let pool = Self {
            config,
            pools: Arc::new(RwLock::new(HashMap::new())),
            connection_semaphore,
            health_check_handle: None,
            metrics,
        };
        
        pool.start_health_check_task();
        pool
    }
    
    fn start_health_check_task(&self) {
        let pools = self.pools.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.health_check_interval);
            
            loop {
                interval.tick().await;
                
                let pools_guard = pools.read().await;
                for (server_id, server_pool) in pools_guard.iter() {
                    // Health check logic here
                    Self::health_check_server_pool(server_id, server_pool, &metrics).await;
                }
            }
        });
        
        // Store handle for cleanup
        // Note: In real implementation, we'd store this properly
    }
    
    async fn health_check_server_pool(
        server_id: &str,
        server_pool: &ServerConnectionPool,
        metrics: &PoolMetrics,
    ) {
        // Implement health check logic
        for (transport, conn_info) in &server_pool.available {
            if transport.is_connected().await {
                metrics.record_healthy_connection(server_id);
            } else {
                metrics.record_unhealthy_connection(server_id);
            }
        }
    }
    
    async fn return_connection(
        &self,
        connection: Box<dyn Transport>,
        info: ConnectionInfo,
    ) -> Result<()> {
        let mut pools = self.pools.write().await;
        if let Some(server_pool) = pools.get_mut(&info.server_id) {
            server_pool.available.push((connection, info));
            server_pool.active_count = server_pool.active_count.saturating_sub(1);
            
            // Notify any waiters
            if let Some(waiter) = server_pool.waiters.pop() {
                if let Some((transport, conn_info)) = server_pool.available.pop() {
                    server_pool.active_count += 1;
                    let pooled = PooledConnection {
                        id: conn_info.id,
                        server_id: conn_info.server_id.clone(),
                        transport,
                        created_at: conn_info.created_at,
                        last_used: Instant::now(),
                        use_count: conn_info.use_count + 1,
                        pool: Arc::new(self.clone()), // Note: Need proper Arc handling
                    };
                    let _ = waiter.send(Ok(pooled));
                }
            }
        }
        
        // Release semaphore permit
        self.connection_semaphore.add_permits(1);
        Ok(())
    }
}

#[async_trait]
impl ConnectionPool for ConnectionPoolImpl {
    async fn acquire(&self, server_id: &str) -> Result<PooledConnection> {
        let start_time = Instant::now();
        
        // Try to acquire semaphore permit
        let _permit = self.connection_semaphore
            .acquire()
            .await
            .map_err(|_| BedrockError::McpError("Connection pool semaphore closed".into()))?;
        
        let mut pools = self.pools.write().await;
        let server_pool = pools.entry(server_id.to_string())
            .or_insert_with(|| ServerConnectionPool::new(server_id.to_string()));
        
        // Try to get available connection
        if let Some((transport, mut conn_info)) = server_pool.available.pop() {
            server_pool.active_count += 1;
            conn_info.last_used = Instant::now();
            conn_info.use_count += 1;
            
            self.metrics.record_acquisition(
                server_id,
                start_time.elapsed().as_millis() as u64,
                false, // not newly created
            );
            
            return Ok(PooledConnection {
                id: conn_info.id,
                server_id: conn_info.server_id,
                transport,
                created_at: conn_info.created_at,
                last_used: conn_info.last_used,
                use_count: conn_info.use_count,
                pool: Arc::new(self.clone()), // Note: Proper Arc handling needed
            });
        }
        
        // No available connections, need to create new one if under limit
        if server_pool.active_count + server_pool.available.len() < self.config.max_connections {
            // Create new connection
            let transport = self.create_new_connection(server_id).await?;
            let conn_info = ConnectionInfo {
                id: Uuid::new_v4(),
                server_id: server_id.to_string(),
                created_at: Instant::now(),
                last_used: Instant::now(),
                use_count: 1,
            };
            
            server_pool.active_count += 1;
            
            self.metrics.record_acquisition(
                server_id,
                start_time.elapsed().as_millis() as u64,
                true, // newly created
            );
            
            return Ok(PooledConnection {
                id: conn_info.id,
                server_id: conn_info.server_id,
                transport,
                created_at: conn_info.created_at,
                last_used: conn_info.last_used,
                use_count: conn_info.use_count,
                pool: Arc::new(self.clone()),
            });
        }
        
        // Pool is full, need to wait
        let (tx, rx) = tokio::sync::oneshot::channel();
        server_pool.waiters.push(tx);
        
        // Release write lock before waiting
        drop(pools);
        
        // Wait for connection with timeout
        match tokio::time::timeout(self.config.acquire_timeout, rx).await {
            Ok(Ok(connection)) => {
                self.metrics.record_acquisition(
                    server_id,
                    start_time.elapsed().as_millis() as u64,
                    false,
                );
                connection
            }
            Ok(Err(_)) => Err(BedrockError::McpError("Connection waiter cancelled".into())),
            Err(_) => Err(BedrockError::McpError("Connection acquisition timeout".into())),
        }
    }
    
    async fn health_status(&self) -> Vec<ConnectionHealth> {
        // Implementation for getting health status
        vec![]
    }
    
    async fn drain(&self, server_id: &str) -> Result<()> {
        let mut pools = self.pools.write().await;
        if let Some(server_pool) = pools.remove(server_id) {
            // Close all connections in the pool
            for (mut transport, _) in server_pool.available {
                let _ = transport.close().await;
            }
        }
        Ok(())
    }
    
    async fn statistics(&self) -> PoolStatistics {
        let pools = self.pools.read().await;
        let mut stats = PoolStatistics {
            total_connections: 0,
            active_connections: 0,
            idle_connections: 0,
            failed_connections: 0,
            total_acquisitions: self.metrics.total_acquisitions.load(std::sync::atomic::Ordering::Relaxed),
            total_releases: self.metrics.total_releases.load(std::sync::atomic::Ordering::Relaxed),
            average_acquisition_time_ms: self.metrics.average_acquisition_time_ms(),
        };
        
        for server_pool in pools.values() {
            stats.active_connections += server_pool.active_count;
            stats.idle_connections += server_pool.available.len();
            stats.failed_connections += server_pool.failed_count;
        }
        
        stats.total_connections = stats.active_connections + stats.idle_connections + stats.failed_connections;
        stats
    }
    
    async fn shutdown(&self) -> Result<()> {
        // Cancel health check task
        if let Some(handle) = &self.health_check_handle {
            handle.abort();
        }
        
        // Drain all pools
        let server_ids: Vec<String> = {
            let pools = self.pools.read().await;
            pools.keys().cloned().collect()
        };
        
        for server_id in server_ids {
            self.drain(&server_id).await?;
        }
        
        Ok(())
    }
}

impl ServerConnectionPool {
    fn new(server_id: String) -> Self {
        Self {
            server_id,
            config: ServerConfiguration::default(),
            available: Vec::new(),
            active_count: 0,
            failed_count: 0,
            waiters: Vec::new(),
        }
    }
}

/// Pool metrics for observability
struct PoolMetrics {
    total_acquisitions: std::sync::atomic::AtomicU64,
    total_releases: std::sync::atomic::AtomicU64,
    acquisition_times: Arc<RwLock<Vec<u64>>>,
    healthy_connections: Arc<RwLock<HashMap<String, usize>>>,
    unhealthy_connections: Arc<RwLock<HashMap<String, usize>>>,
}

impl PoolMetrics {
    fn new() -> Self {
        Self {
            total_acquisitions: std::sync::atomic::AtomicU64::new(0),
            total_releases: std::sync::atomic::AtomicU64::new(0),
            acquisition_times: Arc::new(RwLock::new(Vec::new())),
            healthy_connections: Arc::new(RwLock::new(HashMap::new())),
            unhealthy_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    fn record_acquisition(&self, _server_id: &str, time_ms: u64, _newly_created: bool) {
        self.total_acquisitions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        // Record acquisition time (with circular buffer to prevent memory growth)
        tokio::spawn(async move {
            // Would implement circular buffer here
        });
    }
    
    fn record_healthy_connection(&self, server_id: &str) {
        // Implementation for recording healthy connection
    }
    
    fn record_unhealthy_connection(&self, server_id: &str) {
        // Implementation for recording unhealthy connection
    }
    
    fn average_acquisition_time_ms(&self) -> u64 {
        // Implementation for calculating average acquisition time
        0
    }
}

// Dummy implementations for compilation
use crate::transport::{Transport, JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};

struct NullTransport;

#[async_trait]
impl Transport for NullTransport {
    async fn send_request(&mut self, _request: JsonRpcRequest) -> Result<()> {
        Err(BedrockError::McpError("Null transport".into()))
    }
    
    async fn send_notification(&mut self, _notification: JsonRpcNotification) -> Result<()> {
        Err(BedrockError::McpError("Null transport".into()))
    }
    
    async fn receive_response(&mut self) -> Result<Option<JsonRpcResponse>> {
        Ok(None)
    }
    
    async fn is_connected(&self) -> bool {
        false
    }
    
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ServerConfiguration {
    // Server-specific configuration
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        Self {}
    }
}

impl ConnectionPoolImpl {
    async fn create_new_connection(&self, _server_id: &str) -> Result<Box<dyn Transport>> {
        // Implementation would create actual transport based on server config
        Ok(Box::new(NullTransport))
    }
}

// Note: Proper Clone implementation would be needed for Arc<Self> pattern
impl Clone for ConnectionPoolImpl {
    fn clone(&self) -> Self {
        // This is a simplified clone for demonstration
        // In practice, you'd use Arc<ConnectionPoolImpl> throughout
        Self {
            config: self.config.clone(),
            pools: self.pools.clone(),
            connection_semaphore: self.connection_semaphore.clone(),
            health_check_handle: None,
            metrics: self.metrics.clone(),
        }
    }
}
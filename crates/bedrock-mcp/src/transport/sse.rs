//! SSE (Server-Sent Events) transport implementation for HTTP-based MCP servers

use async_trait::async_trait;
use bedrock_core::{BedrockError, Result};
use reqwest_eventsource::{Event, EventSource};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info};
use futures::StreamExt;

use crate::types::{JsonRpcRequest, JsonRpcResponse, JsonRpcNotification};
use super::Transport;

/// SSE transport for HTTP-based MCP servers
pub struct SseTransport {
    /// Base URL for the SSE endpoint
    url: String,
    
    /// HTTP headers to include in requests
    headers: HashMap<String, String>,
    
    /// HTTP client for sending requests
    client: reqwest::Client,
    
    /// Channel for receiving responses
    response_rx: Arc<Mutex<mpsc::Receiver<JsonRpcResponse>>>,
    
    /// Connection state
    connected: Arc<RwLock<bool>>,
    
    /// Timeout in milliseconds
    #[allow(dead_code)]
    timeout: u64,
    
    /// Discovered messages URL from SSE endpoint event
    messages_url: Arc<RwLock<Option<String>>>,
}

impl std::fmt::Debug for SseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SseTransport")
            .field("url", &self.url)
            .field("connected", &self.connected)
            .finish()
    }
}

impl SseTransport {
    /// Create a new SSE transport
    pub async fn new(
        url: String,
        headers: HashMap<String, String>,
        timeout: u64,
    ) -> Result<Self> {
        info!("Connecting to MCP server via SSE: {}", url);
        
        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(BedrockError::McpError("SSE URL must start with http:// or https://".into()));
        }
        
        // Build HTTP client with headers
        let mut client_builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout));
        
        // Add default headers
        let mut default_headers = reqwest::header::HeaderMap::new();
        for (key, value) in &headers {
            // Resolve environment variable values
            let resolved_value = resolve_env_value(value);
            
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| BedrockError::McpError(format!("Invalid header name '{}': {}", key, e)))?;
            let header_value = reqwest::header::HeaderValue::from_str(&resolved_value)
                .map_err(|e| BedrockError::McpError(format!("Invalid header value for '{}': {}", key, e)))?;
            default_headers.insert(header_name, header_value);
        }
        client_builder = client_builder.default_headers(default_headers);
        
        let client = client_builder.build()
            .map_err(|e| BedrockError::McpError(format!("Failed to build HTTP client: {}", e)))?;
        
        // Create response channel
        let (response_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(100);
        
        // Build SSE request
        let sse_url = if url.ends_with("/sse") {
            url.clone()
        } else {
            format!("{}/sse", url)
        };
        
        let mut request_builder = client.get(&sse_url);
        for (key, value) in &headers {
            let resolved_value = resolve_env_value(value);
            request_builder = request_builder.header(key, resolved_value);
        }
        
        // Start event listener task
        let response_tx_clone = response_tx.clone();
        let connected = Arc::new(RwLock::new(false));
        let connected_clone = connected.clone();
        let messages_url = Arc::new(RwLock::new(None::<String>));
        let messages_url_clone = messages_url.clone();
        let base_url_clone = url.clone();
        
        tokio::spawn(async move {
            info!("Starting SSE event listener for {}", sse_url);
            
            // Create EventSource from the request builder
            let event_source = match EventSource::new(request_builder) {
                Ok(es) => es,
                Err(e) => {
                    error!("Failed to create EventSource: {}", e);
                    return;
                }
            };
            
            let mut stream = event_source;
            
            while let Some(event) = stream.next().await {
                match event {
                    Ok(Event::Open) => {
                        info!("SSE connection opened");
                        *connected_clone.write().await = true;
                    }
                    Ok(Event::Message(msg)) => {
                        // Log event type and data preview
                        let data_preview = if msg.data.len() > 100 {
                            format!("{}...", &msg.data[..100])
                        } else {
                            msg.data.clone()
                        };
                        debug!("SSE Event - Type: '{}', Data: {}", msg.event, data_preview);
                        
                        // Check the event type
                        if msg.event == "endpoint" {
                            // This is an endpoint discovery event
                            let endpoint_url = format!("{}{}", base_url_clone.trim_end_matches('/'), msg.data);
                            info!("Discovered messages endpoint from 'endpoint' event: {}", endpoint_url);
                            *messages_url_clone.write().await = Some(endpoint_url);
                        } else if msg.event == "message" || msg.event.is_empty() {
                            // This is a JSON-RPC message response
                            match serde_json::from_str::<JsonRpcResponse>(&msg.data) {
                                Ok(response) => {
                                    if let Err(e) = response_tx_clone.send(response).await {
                                        error!("Failed to send response through channel: {}", e);
                                    }
                                }
                                Err(e) => {
                                    debug!("Failed to parse message as JSON-RPC response: {} - {}", msg.data, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("SSE stream error: {:?}", e);
                        *connected_clone.write().await = false;
                        break;
                    }
                }
            }
            
            info!("SSE event listener ended");
            *connected_clone.write().await = false;
        });
        
        // Wait briefly for connection to establish
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        Ok(Self {
            url,
            headers,
            client,
            response_rx: Arc::new(Mutex::new(response_rx)),
            connected,
            timeout,
            messages_url,
        })
    }
    
    /// Send a message via HTTP POST to the messages endpoint
    async fn send_message(&self, json: String) -> Result<()> {
        // Get the messages URL
        let messages_url = {
            let url_guard = self.messages_url.read().await;
            if let Some(url) = url_guard.as_ref() {
                url.clone()
            } else {
                // Default to /messages endpoint
                format!("{}/messages", self.url.trim_end_matches('/'))
            }
        };
        
        debug!("Sending message to {}: {}", messages_url, json);
        
        // Build request with headers
        let mut request = self.client.post(&messages_url)
            .header("Content-Type", "application/json");
        
        for (key, value) in &self.headers {
            let resolved_value = resolve_env_value(value);
            request = request.header(key, resolved_value);
        }
        
        // Send the request
        let response = request
            .body(json)
            .send()
            .await
            .map_err(|e| BedrockError::McpError(format!("Failed to send HTTP request: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "".to_string());
            return Err(BedrockError::McpError(format!("HTTP request failed with status {}: {}", status, body)));
        }
        
        Ok(())
    }
}

#[async_trait]
impl Transport for SseTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<()> {
        let json = serde_json::to_string(&request)
            .map_err(|e| BedrockError::SerializationError(e))?;
        
        self.send_message(json).await
    }
    
    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let json = serde_json::to_string(&notification)
            .map_err(|e| BedrockError::SerializationError(e))?;
        
        self.send_message(json).await
    }
    
    async fn receive_response(&mut self) -> Result<Option<JsonRpcResponse>> {
        let mut rx_guard = self.response_rx.lock().await;
        Ok(rx_guard.recv().await)
    }
    
    async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    async fn close(&mut self) -> Result<()> {
        info!("Closing SSE transport");
        *self.connected.write().await = false;
        Ok(())
    }
}

/// Resolve environment variable values (including secret patterns)
fn resolve_env_value(value: &str) -> String {
    if value.starts_with("${") && value.ends_with("}") {
        let inner = &value[2..value.len()-1];
        
        // Handle different secret patterns
        if let Some((source, path)) = inner.split_once(':') {
            match source {
                "env" => {
                    // ${env:VAR_NAME}
                    std::env::var(path).unwrap_or_else(|_| value.to_string())
                }
                "file" => {
                    // ${file:/path/to/secret}
                    std::fs::read_to_string(path)
                        .unwrap_or_else(|_| value.to_string())
                        .trim()
                        .to_string()
                }
                _ => {
                    // Unknown source, try as plain env var
                    if let Some((var_name, default)) = inner.split_once(":-") {
                        std::env::var(var_name).unwrap_or_else(|_| default.to_string())
                    } else {
                        std::env::var(inner).unwrap_or_else(|_| value.to_string())
                    }
                }
            }
        } else {
            // Handle ${VAR:-default} pattern
            if let Some((var_name, default)) = inner.split_once(":-") {
                std::env::var(var_name).unwrap_or_else(|_| default.to_string())
            } else {
                std::env::var(inner).unwrap_or_else(|_| value.to_string())
            }
        }
    } else {
        value.to_string()
    }
}
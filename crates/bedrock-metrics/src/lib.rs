use bedrock_config::{AgentConfig, ModelPricing};
use bedrock_core::{CostDetails, TokenStatistics};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tracing::debug;

pub struct TokenTracker {
    input_tokens: AtomicUsize,
    output_tokens: AtomicUsize,
    cache_tokens: AtomicUsize,
    model_stats: Arc<RwLock<HashMap<String, ModelTokenStats>>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelTokenStats {
    pub model_id: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_tokens: usize,
    pub requests: usize,
}

impl TokenTracker {
    pub fn new() -> Self {
        Self {
            input_tokens: AtomicUsize::new(0),
            output_tokens: AtomicUsize::new(0),
            cache_tokens: AtomicUsize::new(0),
            model_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_input(&self, tokens: usize, model: &str) {
        self.input_tokens.fetch_add(tokens, Ordering::Relaxed);
        
        let mut stats = self.model_stats.write().unwrap();
        let model_stat = stats.entry(model.to_string()).or_insert_with(|| {
            ModelTokenStats {
                model_id: model.to_string(),
                ..Default::default()
            }
        });
        model_stat.input_tokens += tokens;
        model_stat.requests += 1;
    }

    pub fn add_output(&self, tokens: usize, model: &str) {
        self.output_tokens.fetch_add(tokens, Ordering::Relaxed);
        
        let mut stats = self.model_stats.write().unwrap();
        let model_stat = stats.entry(model.to_string()).or_insert_with(|| {
            ModelTokenStats {
                model_id: model.to_string(),
                ..Default::default()
            }
        });
        model_stat.output_tokens += tokens;
    }

    pub fn add_cache_hit(&self, tokens: usize, model: &str) {
        self.cache_tokens.fetch_add(tokens, Ordering::Relaxed);
        
        let mut stats = self.model_stats.write().unwrap();
        let model_stat = stats.entry(model.to_string()).or_insert_with(|| {
            ModelTokenStats {
                model_id: model.to_string(),
                ..Default::default()
            }
        });
        model_stat.cache_tokens += tokens;
    }

    pub fn get_stats(&self) -> TokenStatistics {
        TokenStatistics {
            input_tokens: self.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.output_tokens.load(Ordering::Relaxed),
            total_tokens: self.input_tokens.load(Ordering::Relaxed) 
                + self.output_tokens.load(Ordering::Relaxed),
            cache_hits: self.cache_tokens.load(Ordering::Relaxed),
        }
    }

    pub fn get_model_stats(&self) -> HashMap<String, ModelTokenStats> {
        self.model_stats.read().unwrap().clone()
    }

    pub fn reset(&self) {
        self.input_tokens.store(0, Ordering::Relaxed);
        self.output_tokens.store(0, Ordering::Relaxed);
        self.cache_tokens.store(0, Ordering::Relaxed);
        self.model_stats.write().unwrap().clear();
    }
}

pub struct CostCalculator {
    pricing: HashMap<String, ModelPricing>,
    currency: String,
    budget_limit: Option<f64>,
    alert_threshold: f64,
    total_cost: Arc<RwLock<f64>>,
}

impl CostCalculator {
    pub fn from_config(config: &AgentConfig) -> Self {
        Self {
            pricing: config.pricing.clone(),
            currency: "USD".to_string(),
            budget_limit: config.limits.budget_limit,
            alert_threshold: config.limits.alert_threshold,
            total_cost: Arc::new(RwLock::new(0.0)),
        }
    }

    pub fn calculate(&self, tokens: &TokenStatistics, model: &str) -> CostDetails {
        let pricing = self.pricing.get(model);
        
        match pricing {
            Some(p) => {
                let input_cost = (tokens.input_tokens as f64 / 1000.0) * p.input_per_1k;
                let output_cost = (tokens.output_tokens as f64 / 1000.0) * p.output_per_1k;
                let total = input_cost + output_cost;
                
                {
                    let mut total_cost = self.total_cost.write().unwrap();
                    *total_cost += total;
                }
                
                CostDetails {
                    input_cost,
                    output_cost,
                    total_cost: total,
                    currency: p.currency.clone(),
                    model: model.to_string(),
                }
            }
            None => {
                debug!("No pricing found for model: {}", model);
                CostDetails {
                    model: model.to_string(),
                    currency: self.currency.clone(),
                    ..Default::default()
                }
            }
        }
    }

    pub fn check_budget(&self) -> BudgetStatus {
        let current_cost = *self.total_cost.read().unwrap();
        
        match self.budget_limit {
            Some(limit) => {
                if current_cost >= limit {
                    BudgetStatus::Exceeded { 
                        amount: current_cost - limit 
                    }
                } else if current_cost >= limit * self.alert_threshold {
                    BudgetStatus::Warning { 
                        remaining: limit - current_cost 
                    }
                } else {
                    BudgetStatus::Ok
                }
            }
            None => BudgetStatus::Ok,
        }
    }

    pub fn get_total_cost(&self) -> f64 {
        *self.total_cost.read().unwrap()
    }

    pub fn reset(&self) {
        *self.total_cost.write().unwrap() = 0.0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetStatus {
    Ok,
    Warning { remaining: f64 },
    Exceeded { amount: f64 },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsCollector {
    pub requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub total_latency_ms: u64,
    pub tool_executions: HashMap<String, ToolMetrics>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolMetrics {
    pub name: String,
    pub executions: usize,
    pub failures: usize,
    pub total_duration_ms: u64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            started_at: Some(Utc::now()),
            ..Default::default()
        }
    }

    pub fn record_request(&mut self, duration_ms: u64, success: bool) {
        self.requests += 1;
        self.total_latency_ms += duration_ms;
        
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
    }

    pub fn record_tool_execution(&mut self, tool: &str, duration_ms: u64, success: bool) {
        let metrics = self.tool_executions.entry(tool.to_string())
            .or_insert_with(|| ToolMetrics {
                name: tool.to_string(),
                ..Default::default()
            });
        
        metrics.executions += 1;
        metrics.total_duration_ms += duration_ms;
        
        if !success {
            metrics.failures += 1;
        }
    }

    pub fn get_average_latency(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.total_latency_ms as f64 / self.requests as f64
        }
    }

    pub fn get_success_rate(&self) -> f64 {
        if self.requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.requests as f64 * 100.0
        }
    }

    pub fn get_summary(&self) -> MetricsSummary {
        MetricsSummary {
            total_requests: self.requests,
            success_rate: self.get_success_rate(),
            average_latency_ms: self.get_average_latency(),
            uptime_seconds: self.started_at
                .map(|start| (Utc::now() - start).num_seconds() as u64)
                .unwrap_or(0),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_requests: usize,
    pub success_rate: f64,
    pub average_latency_ms: f64,
    pub uptime_seconds: u64,
}

pub fn estimate_tokens(text: &str, model: &str) -> usize {
    let chars_per_token = if model.contains("claude") {
        3.5
    } else {
        4.0
    };
    
    (text.len() as f64 / chars_per_token).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_tracker() {
        let tracker = TokenTracker::new();
        
        tracker.add_input(100, "claude-3");
        tracker.add_output(50, "claude-3");
        tracker.add_cache_hit(25, "claude-3");
        
        let stats = tracker.get_stats();
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
        assert_eq!(stats.total_tokens, 150);
        assert_eq!(stats.cache_hits, 25);
        
        let model_stats = tracker.get_model_stats();
        assert_eq!(model_stats.get("claude-3").unwrap().requests, 1);
    }

    #[test]
    fn test_metrics_collector() {
        let mut collector = MetricsCollector::new();
        
        collector.record_request(100, true);
        collector.record_request(200, true);
        collector.record_request(150, false);
        
        assert_eq!(collector.requests, 3);
        assert_eq!(collector.successful_requests, 2);
        assert_eq!(collector.failed_requests, 1);
        assert_eq!(collector.get_average_latency(), 150.0);
        assert!((collector.get_success_rate() - 66.67).abs() < 0.01);
    }

    #[test]
    fn test_token_estimation() {
        let text = "This is a test message";
        let tokens = estimate_tokens(text, "claude-3");
        assert!(tokens > 0);
        assert!(tokens < text.len());
    }
}
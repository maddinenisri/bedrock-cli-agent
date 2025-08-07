# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-metrics** - Token tracking, cost calculation, and usage metrics collection. Provides thread-safe counters, budget monitoring, and performance statistics for the agent system.

## Key Components

### TokenTracker
Thread-safe token counting:
```rust
pub struct TokenTracker {
    input_tokens: Arc<AtomicUsize>,
    output_tokens: Arc<AtomicUsize>,
    cache_hits: Arc<AtomicUsize>,
    total_requests: Arc<AtomicUsize>,
}
```

### CostCalculator
Budget monitoring and alerts:
```rust
pub struct CostCalculator {
    pricing: HashMap<String, ModelPricing>,
    total_cost: Arc<Mutex<f64>>,
    budget_limit: Option<f64>,
    alert_threshold: f64,  // e.g., 0.8 = 80%
}
```

### MetricsCollector
Performance and usage stats:
```rust
pub struct MetricsCollector {
    start_time: Instant,
    successful_requests: Arc<AtomicUsize>,
    failed_requests: Arc<AtomicUsize>,
    tool_executions: Arc<AtomicUsize>,
    model_stats: Arc<RwLock<HashMap<String, ModelTokenStats>>>,
}
```

## Development Guidelines

### Token Tracking
```rust
// Increment counters
tracker.add_input_tokens(count);
tracker.add_output_tokens(count);
tracker.increment_cache_hits();

// Get current stats
let stats = tracker.get_statistics();
```

### Cost Calculation
```rust
// Calculate cost for tokens
let cost = calculator.calculate_cost(
    model,
    input_tokens,
    output_tokens
)?;

// Check budget
if calculator.is_over_budget() {
    // Handle budget exceeded
}
```

### Testing Commands
```bash
cargo test -p bedrock-metrics           # All tests
cargo test -p bedrock-metrics test_cost # Cost calculation tests
```

## Important Implementation Details

### Atomic Operations
Use atomic types for thread-safe counters:
```rust
self.input_tokens.fetch_add(count, Ordering::Relaxed);
```

### Budget Monitoring
```rust
pub fn check_budget(&self, new_cost: f64) -> BudgetStatus {
    let current = *self.total_cost.lock().unwrap();
    let projected = current + new_cost;
    
    match self.budget_limit {
        Some(limit) => {
            if projected > limit {
                BudgetStatus::Exceeded
            } else if projected > limit * self.alert_threshold {
                BudgetStatus::Warning
            } else {
                BudgetStatus::Ok
            }
        }
        None => BudgetStatus::Ok
    }
}
```

### Per-Model Statistics
```rust
pub struct ModelTokenStats {
    pub model_id: String,
    pub total_input: usize,
    pub total_output: usize,
    pub request_count: usize,
    pub total_cost: f64,
    pub last_used: DateTime<Utc>,
}
```

## Common Patterns

### Creating Metrics Components
```rust
// Token tracker
let tracker = TokenTracker::new();

// Cost calculator with pricing
let mut pricing = HashMap::new();
pricing.insert(
    "claude-3-5-sonnet".to_string(),
    ModelPricing {
        input_per_1k: 0.003,
        output_per_1k: 0.015,
        currency: "USD".to_string(),
    }
);
let calculator = CostCalculator::new(pricing, Some(10.0), 0.8);

// Metrics collector
let collector = MetricsCollector::new();
```

### Tracking Request Metrics
```rust
// Start of request
collector.start_request();

// After completion
if success {
    collector.record_success();
} else {
    collector.record_failure();
}

// Tool execution
collector.record_tool_execution("tool_name");
```

### Getting Statistics
```rust
// Overall stats
let stats = collector.get_summary();
println!("Success rate: {:.2}%", stats.success_rate * 100.0);
println!("Uptime: {:?}", stats.uptime);

// Model-specific stats
let model_stats = collector.get_model_stats("claude-3-5-sonnet");
```

## Pricing Configuration

Default pricing for common models:
```rust
pub fn default_pricing() -> HashMap<String, ModelPricing> {
    let mut pricing = HashMap::new();
    
    // Claude 3.5 Sonnet
    pricing.insert(
        "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
        ModelPricing {
            input_per_1k: 0.003,
            output_per_1k: 0.015,
            currency: "USD".to_string(),
        }
    );
    
    // Add more models...
    pricing
}
```

## Performance Metrics

### Available Metrics
- Request latency (p50, p95, p99)
- Success/failure rates
- Token throughput (tokens/second)
- Cost per request
- Tool execution counts
- Cache hit rates

### Export Formats
```rust
// JSON export
let json = collector.export_json()?;

// Prometheus format (future)
let prometheus = collector.export_prometheus()?;
```

## Architecture Notes

### Thread Safety
All metrics use atomic operations or mutex protection:
- `AtomicUsize` for counters
- `Arc<Mutex>` for floats
- `Arc<RwLock>` for collections

### Memory Efficiency
- Fixed-size atomic counters
- Model stats pruned periodically
- Circular buffer for latency samples (future)

### Integration Points
- Used by `bedrock-client` for token tracking
- Used by `bedrock-agent` for cost calculation
- Used by `bedrock-task` for performance monitoring

## Dependencies to Note

- `chrono`: Timestamp tracking
- `serde`: Metrics serialization
- Standard library atomics: Thread-safe counters
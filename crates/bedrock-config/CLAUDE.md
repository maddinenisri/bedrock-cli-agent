# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this crate.

## Crate Purpose

**bedrock-config** - Configuration management with YAML parsing and environment variable substitution. Provides centralized configuration for agent settings, AWS credentials, tool permissions, and pricing.

## Key Components

### Configuration Structure
- **AgentConfig**: Root configuration containing all settings
- **AgentSettings**: Model selection, temperature, max tokens
- **AwsSettings**: Region, profile, role configuration
- **ToolSettings**: Allowed tools and permission levels
- **ModelPricing**: Cost per 1k tokens for input/output
- **PathSettings**: Workspace and home directory paths

### Environment Variable Substitution
```rust
// Supports two patterns:
${VARIABLE}          // Simple substitution
${VARIABLE:-default} // With default value
```

The regex pattern handles nested JSON structures recursively.

## Development Guidelines

### Adding Configuration Fields
1. Add field to appropriate struct with `#[serde(default)]`
2. Implement custom default if needed
3. Update example configs
4. Add validation if required

### Testing Commands
```bash
cargo test -p bedrock-config                    # Run tests
cargo test -p bedrock-config test_env_var      # Test env substitution
```

## Important Implementation Details

### Environment Variable Processing
```rust
// The regex for variable substitution
static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").unwrap()
});
```

### Configuration Loading Flow
1. Load YAML from file or string
2. Apply environment variable substitution recursively
3. Parse into strongly-typed structures
4. Apply defaults for missing fields
5. Validate required fields

### Default Values
- Model: `anthropic.claude-3-5-sonnet-20241022-v2:0`
- Temperature: 0.7
- Max tokens: 4096
- Region: us-east-1
- Workspace: `./workspace`

## Configuration Schema

```yaml
agent:
  name: "agent-name"
  model: "model-id"
  temperature: 0.0-1.0
  max_tokens: integer
  system_prompt: "optional"

aws:
  region: "aws-region"
  profile: "optional-profile"
  role_arn: "optional-role"

tools:
  allowed: [tool_names]
  permissions:
    tool_name:
      permission: allow|ask|deny
      constraint: "description"

pricing:
  "model-id":
    input_per_1k: float
    output_per_1k: float
    currency: "USD"

paths:
  home_dir: "${HOME_DIR:-~/.bedrock-agent}"
  workspace_dir: "${WORKSPACE_DIR:-./workspace}"
```

## Common Patterns

### Loading Configuration
```rust
let config = AgentConfig::from_file("config.yaml")?;
// or
let config = AgentConfig::from_str(&yaml_content)?;
```

### Accessing Settings
```rust
let model = &config.agent.model;
let region = config.aws.region.clone();
let allowed_tools = &config.tools.allowed;
```

### Checking Tool Permissions
```rust
if config.tools.allowed.contains(&"execute_bash".to_string()) {
    // Tool is allowed
}
```

## Error Handling

Configuration errors are wrapped in `BedrockError::ConfigError` with context:
- Missing required fields
- Invalid YAML syntax
- File I/O errors
- Invalid values

## Dependencies to Note

- `serde_yaml`: YAML parsing
- `regex` + `once_cell`: Environment variable patterns
- `dirs`: Cross-platform home directory
- `bedrock-core`: Error types
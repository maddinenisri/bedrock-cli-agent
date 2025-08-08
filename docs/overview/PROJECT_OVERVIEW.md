# Bedrock CLI Agent - Project Overview

## What is Bedrock CLI Agent?

The Bedrock CLI Agent is a Rust-based command-line tool that provides an intelligent agent interface to AWS Bedrock's Large Language Models (LLMs). It enables users to interact with Claude and other models through a feature-rich CLI, complete with built-in tools for file operations, code searching, and command execution.

## Key Features

### 🤖 LLM Integration
- Direct integration with AWS Bedrock's Converse API
- Support for Claude 3.5 Sonnet and other Bedrock models
- Full streaming support with real-time responses
- Tool-augmented conversations for extended capabilities

### 🛠️ Built-in Tool System
- **File Operations**: Read, write, and list files with security sandboxing
- **Search Capabilities**: Pattern matching with grep, find, and ripgrep
- **Command Execution**: Safe bash command execution with timeout controls
- **Extensible Architecture**: Easy to add custom tools

### 📊 Observability & Metrics
- Token usage tracking (input/output)
- Cost calculation per request
- Request latency monitoring
- Comprehensive error reporting

### 🔒 Security & Safety
- Workspace sandboxing for file operations
- Path traversal protection
- Command execution timeouts
- Configurable tool permissions
- AWS credential chain support

### ⚙️ Configuration
- YAML-based configuration
- Environment variable substitution
- Per-model pricing configuration
- Flexible tool permission settings

## Architecture Overview

The project follows a modular crate architecture:

```
bedrock-cli-agent/
├── bedrock-core/       # Core types and traits
├── bedrock-client/     # AWS Bedrock client
├── bedrock-config/     # Configuration management
├── bedrock-tools/      # Tool implementations
├── bedrock-task/       # Task execution engine
├── bedrock-agent/      # Agent orchestration
├── bedrock-metrics/    # Metrics and monitoring
└── bedrock-mcp/        # MCP integration (partial)
```

## Use Cases

### 1. Interactive AI Assistant
```bash
bedrock-agent chat --system "You are a helpful coding assistant"
```

### 2. Automated Task Execution
```bash
bedrock-agent task --prompt "Analyze all Python files and create a summary"
```

### 3. Code Analysis & Generation
```bash
bedrock-agent task --prompt "Review this code for security issues" --context "Focus on SQL injection"
```

### 4. File Management
```bash
bedrock-agent task --prompt "Organize these files by type and create a report"
```

## Technology Stack

- **Language**: Rust (for performance and safety)
- **Async Runtime**: Tokio
- **AWS SDK**: aws-sdk-bedrockruntime
- **Serialization**: Serde (JSON/YAML)
- **CLI Framework**: Clap
- **Logging**: Tracing

## Project Status

The project is **87.5% complete** with core functionality working:
- ✅ Core infrastructure (100%)
- ✅ Tool system (100%)
- 🔄 AWS Bedrock integration (50% - missing caching and rate limiting)
- ⚠️ MCP integration (has critical issues)

## Design Principles

1. **Safety First**: All operations are sandboxed and validated
2. **Performance**: Async/await throughout for efficiency
3. **Extensibility**: Plugin-based tool system
4. **Observability**: Comprehensive metrics and logging
5. **User Experience**: Clear error messages and progress indicators

## Comparison with Alternatives

| Feature | Bedrock CLI Agent | Direct API | Other CLIs |
|---------|------------------|------------|------------|
| Built-in Tools | ✅ Extensive | ❌ None | ⚠️ Limited |
| Streaming | ✅ Full support | ⚠️ Complex | ⚠️ Varies |
| Cost Tracking | ✅ Built-in | ❌ Manual | ❌ Rare |
| Security | ✅ Sandboxed | ⚠️ DIY | ⚠️ Varies |
| AWS Integration | ✅ Native | ✅ Native | ❌ Often missing |

## Future Vision

### Short Term (1-2 months)
- Complete caching implementation for cost optimization
- Add rate limiting for production safety
- Fix MCP integration issues

### Medium Term (3-6 months)
- Multi-model support (GPT, Gemini via adapters)
- Advanced caching strategies
- Distributed execution support

### Long Term (6-12 months)
- Web UI for non-technical users
- Plugin marketplace for tools
- Enterprise features (audit, compliance)

## Getting Started

1. **Install**: Build from source with `cargo build --release`
2. **Configure**: Set up AWS credentials and create config.yaml
3. **Test**: Run `bedrock-agent test` to verify connectivity
4. **Use**: Start with `bedrock-agent chat` for interactive mode

## Community & Support

- **Documentation**: Comprehensive guides in `/docs`
- **Examples**: Sample code in `/examples`
- **Issues**: Report bugs via GitHub Issues
- **Contributing**: See CONTRIBUTING.md

## License

MIT OR Apache-2.0 (dual-licensed for maximum compatibility)

---

*The Bedrock CLI Agent brings the power of AWS Bedrock LLMs to your command line with safety, observability, and extensibility at its core.*
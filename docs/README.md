# Bedrock CLI Agent - Documentation

## 📚 Documentation Structure

This documentation is organized to provide easy access to all project information, from high-level overviews to detailed technical specifications.

## Quick Links

- 🚀 [Getting Started](guides/getting-started.md)
- 📊 [Project Status](status/EPIC_STATUS.md)
- ⚠️ [Known Issues](implementation/mcp/known-issues.md)
- 🔧 [Configuration Guide](guides/configuration.md)

## Documentation Categories

### 📋 Project Overview
- [Project Overview](overview/PROJECT_OVERVIEW.md) - High-level project description
- [Architecture](overview/ARCHITECTURE.md) - System architecture and design
- [Security](overview/SECURITY.md) - Security considerations and best practices
- [Roadmap](overview/ROADMAP.md) - Future plans and enhancements

### 📊 Status & Tracking
- [EPIC Status](status/EPIC_STATUS.md) - Comprehensive status of all project epics
- [Implementation Status](status/IMPLEMENTATION_STATUS.md) - Current implementation state
- [Known Issues](status/KNOWN_ISSUES.md) - Active issues and limitations

### 📖 Epic Documentation
Detailed documentation for each major project epic:
- [Epic 1: Core Infrastructure](epics/epic1-core-infrastructure.md)
- [Epic 2: AWS Bedrock Integration](epics/epic2-aws-bedrock.md)
- [Epic 3: Tool System](epics/epic3-tool-system.md)
- [Epic 4: MCP Integration](epics/epic4-mcp-integration.md)

### 🔧 Implementation Details

#### MCP (Model Context Protocol)
- [MCP Overview](implementation/mcp/README.md) - Current status and overview
- [Implementation Status](implementation/mcp/implementation-status.md) - Technical implementation details
- [Known Issues](implementation/mcp/known-issues.md) - Critical issues and fixes needed
- [Integration Guide](implementation/mcp/integration-guide.md) - How to use MCP (when ready)
- [Improvement Roadmap](implementation/mcp/improvement-roadmap.md) - Path to completion

#### Caching System
- [Cache Design](implementation/caching/README.md) - LRU cache implementation (planned)
- [Implementation Plan](implementation/caching/implementation-plan.md) - Roadmap for cache features

#### Rate Limiting
- [Rate Limiting Design](implementation/rate-limiting/README.md) - Rate limiting system (planned)
- [Implementation Plan](implementation/rate-limiting/implementation-plan.md) - Development roadmap

### 🏗️ Design Documents
Architecture decisions and design patterns:
- [Architecture Decisions](design/architecture-decisions.md) - Key architectural choices
- [Configuration Management](design/configuration-management.md) - Config system design
- [Error Recovery](design/error-recovery.md) - Error handling strategies
- [Message Routing](design/message-routing.md) - Message flow architecture
- [Observability](design/observability.md) - Monitoring and logging design
- [Tool Registry](design/tool-registry.md) - Tool system architecture
- [Transport Layer](design/transport-layer.md) - Communication layer design

### 📚 User Guides
Practical guides for users and developers:
- [Getting Started](guides/getting-started.md) - Quick start guide
- [Configuration](guides/configuration.md) - Detailed configuration options
- [Development](guides/development.md) - Development setup and guidelines
- [Troubleshooting](guides/troubleshooting.md) - Common issues and solutions
- [Examples](guides/examples.md) - Usage examples and patterns

### 🔌 API Documentation
Technical API documentation for each crate:
- [bedrock-core](api/crates/bedrock-core.md) - Core types and traits
- [bedrock-client](api/crates/bedrock-client.md) - AWS Bedrock client
- [bedrock-config](api/crates/bedrock-config.md) - Configuration management
- [bedrock-tools](api/crates/bedrock-tools.md) - Tool system
- [bedrock-task](api/crates/bedrock-task.md) - Task execution
- [bedrock-agent](api/crates/bedrock-agent.md) - Agent orchestration
- [bedrock-metrics](api/crates/bedrock-metrics.md) - Metrics and monitoring
- [bedrock-mcp](api/crates/bedrock-mcp.md) - MCP integration

## 🎯 Current Project Status

### Overall Progress: 87.5% Complete

| Epic | Status | Completion |
|------|--------|------------|
| Epic 1: Core Infrastructure | ✅ Complete | 100% (4/4 stories) |
| Epic 2: AWS Bedrock Integration | 🔄 In Progress | 50% (2/4 stories) |
| Epic 3: Tool System | ✅ Complete | 100% (4/4 stories) |
| Epic 4: MCP Integration | ⚠️ Has Issues | Code exists but critical bugs |

### Priority Focus Areas

1. **🚨 Critical**: Fix MCP tool interface incompatibility
2. **📊 High**: Implement caching layer (Epic 2.3)
3. **🔒 High**: Implement rate limiting (Epic 2.4)
4. **🔧 Medium**: Resolve MCP known issues

## 🗺️ Navigation Tips

- **New Users**: Start with [Getting Started](guides/getting-started.md)
- **Developers**: Check [Development Guide](guides/development.md) and [API Docs](api/crates/)
- **Project Status**: See [EPIC Status](status/EPIC_STATUS.md)
- **Troubleshooting**: Visit [Known Issues](status/KNOWN_ISSUES.md) and [Troubleshooting Guide](guides/troubleshooting.md)

## 📝 Documentation Standards

- All documentation uses Markdown format
- Code examples include language hints for syntax highlighting
- Status indicators: ✅ Complete, 🔄 In Progress, ⚠️ Has Issues, 📋 Planned
- Cross-references use relative links for portability

## 🔄 Recent Updates

- Consolidated MCP documentation to clarify actual vs claimed status
- Created unified EPIC status tracker
- Reorganized documentation structure for better navigation
- Updated README to reflect actual implementation status

## 📮 Contributing to Documentation

When updating documentation:
1. Maintain consistent formatting
2. Update cross-references if moving files
3. Keep status indicators current
4. Add entries to relevant index files
5. Test all links before committing

---

*Last Updated: Current repository state*  
*Version: 1.0.0*
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Unified CLI command structure with four main command groups
- `conversation` command for managing conversations (resume, summary, export, delete)
- `task` command for executing and managing tasks
- `import` command for importing conversations and tasks from JSON
- `list` command for listing conversations, tasks, and showing statistics
- AI-powered conversation summary generation
- Task continuation with context preservation
- Auto-detection of UUIDs vs prompts in task command
- Auto-detection of import type (conversation vs task)
- Comprehensive conversation statistics tracking
- Export functionality for tasks
- Import with immediate resume option
- Verbose listing mode for detailed output
- Migration guide for users upgrading from older versions

### Changed
- **BREAKING**: Restructured CLI commands into logical groups
- **BREAKING**: `resume` is now `conversation <id>`
- **BREAKING**: `list-conversations` is now `list`
- **BREAKING**: `export-conversation` is now `conversation <id> --export`
- **BREAKING**: `delete-conversation` is now `conversation <id> --delete`
- **BREAKING**: `conversation-stats` is now `list --stats`
- Improved command discoverability with grouped help
- Better error messages with context
- Optimized code organization with unified handlers

### Fixed
- Fixed unused variable warnings in release build
- Fixed conversation resumption with proper history loading
- Improved error handling for missing conversations

## [0.1.0] - 2024-01-08

### Added
- Initial release with core functionality
- AWS Bedrock LLM interaction via Converse API
- Full streaming support with tool execution
- Built-in file operation tools (read, write, list)
- Search tools (grep, find, ripgrep)
- Bash command execution with safety controls
- Task processing with UUID-based tracking
- Token statistics and cost tracking
- YAML-based configuration with environment variable substitution
- Modular crate architecture
- MCP tool integration (stdio/SSE)
- Basic conversation persistence with JSONL format
- Workspace-based conversation organization
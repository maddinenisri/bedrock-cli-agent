# Security Guidelines for Bedrock CLI Agent

## Overview

This document outlines security best practices and guidelines for using and deploying the Bedrock CLI Agent in production environments.

## Configuration Security

### API Keys and Secrets

**NEVER commit API keys, tokens, or secrets to version control.**

1. **Use environment variables** for all sensitive values:
   ```bash
   export AWS_ACCESS_KEY_ID="your-key-id"
   export AWS_SECRET_ACCESS_KEY="your-secret-key"
   export GITHUB_TOKEN="your-github-token"
   export FIGMA_API_KEY="your-figma-key"
   ```

2. **Use the example configuration** as a template:
   ```bash
   cp config.yaml.example config.yaml
   # Edit config.yaml with your actual values
   ```

3. **The `.gitignore` file excludes**:
   - `config.yaml` - Your actual configuration
   - `.env` files - Environment variables
   - `mcp-servers.yaml` - MCP server configurations

### Secret Management Best Practices

For production deployments, use proper secret management:

- **AWS Secrets Manager**: Store and rotate secrets automatically
- **HashiCorp Vault**: Enterprise secret management
- **Kubernetes Secrets**: For containerized deployments
- **Azure Key Vault**: For Azure deployments

Example AWS Secrets Manager integration:
```rust
// Future enhancement - retrieve secrets at runtime
let secret = aws_secrets_manager::get_secret("bedrock-agent/api-keys").await?;
```

## Command Execution Security

### Command Validation

The agent includes a comprehensive command validation system:

1. **Dangerous Pattern Detection**: Blocks commands containing:
   - Destructive operations (`rm -rf`, `mkfs`, etc.)
   - Privilege escalation (`sudo`, `su`, etc.)
   - Reverse shells and remote execution
   - Credential theft attempts

2. **Strict Mode**: Enable to allow only whitelisted commands:
   ```yaml
   tools:
     permissions:
       execute_bash:
         permission: ask
         constraint: "Only allow safe read-only commands"
   ```

3. **Safe Commands Whitelist**:
   - Read-only filesystem operations: `ls`, `cat`, `grep`, `find`
   - Information gathering: `pwd`, `date`, `whoami`
   - Development tools: `git status`, `cargo test`, `npm list`

### Sandboxing

All file operations are sandboxed to the workspace directory:

```yaml
paths:
  workspace_dir: "${WORKSPACE_DIR:-./workspace}"
```

The agent validates all paths to prevent directory traversal attacks.

## MCP Server Security

### Transport Security

1. **SSE Transport**: Always use HTTPS in production:
   ```yaml
   mcp:
     inline_servers:
       api-server:
         type: "sse"
         url: "https://api.example.com"  # Use HTTPS
         headers:
           Authorization: "Bearer ${API_TOKEN}"
   ```

2. **Stdio Transport**: Validate server binaries:
   - Only use trusted MCP server implementations
   - Verify checksums of downloaded servers
   - Run servers with minimal privileges

### Authentication & Authorization

1. **Token Management**:
   - Rotate API tokens regularly
   - Use short-lived tokens when possible
   - Implement token refresh mechanisms

2. **Network Restrictions**:
   ```yaml
   security:
     allowed_domains:
       - "github.com"
       - "api.github.com"
       - "your-trusted-domain.com"
   ```

## Input Validation

### Tool Arguments

All tool inputs are validated:
- Type checking against JSON schemas
- Size limits for file operations
- Pattern validation for search operations
- Command sanitization for execution

### File Operations

1. **Size Limits**: Default 10MB max file size
2. **Extension Filtering**: Whitelist allowed file types
3. **Path Validation**: Prevent directory traversal
4. **UTF-8 Validation**: Ensure text file encoding

## Output Security

### Secret Scanning

Enable secret scanning to prevent accidental exposure:

```yaml
security:
  scan_secrets: true
  redact_patterns:
    - "(?i)(api[_-]?key|token|secret|password)\\s*[:=]\\s*['\"]?([^'\"\\s]+)"
```

### Audit Logging

Enable comprehensive audit logging:

```yaml
security:
  audit_logging: true
```

Audit logs include:
- All tool executions with parameters
- Command execution attempts
- File operations
- MCP server communications

## Rate Limiting

Protect against abuse:

```yaml
rate_limit:
  max_requests_per_minute: 60
  max_tokens_per_minute: 100000
```

## Deployment Security

### Container Security

1. **Use minimal base images**:
   ```dockerfile
   FROM rust:1.75-slim AS builder
   # Build stage
   
   FROM debian:bookworm-slim
   # Runtime with minimal dependencies
   ```

2. **Run as non-root user**:
   ```dockerfile
   RUN useradd -m -u 1001 bedrock-agent
   USER bedrock-agent
   ```

3. **Security scanning**:
   ```bash
   # Scan for vulnerabilities
   cargo audit
   trivy image bedrock-agent:latest
   ```

### Network Security

1. **TLS/SSL**: Always use encrypted connections
2. **Firewall Rules**: Restrict inbound/outbound traffic
3. **Private Networks**: Deploy in VPC/private subnets
4. **Service Mesh**: Use Istio/Linkerd for zero-trust networking

## Monitoring & Alerting

### Security Monitoring

Monitor for suspicious activity:
- Failed authentication attempts
- Unusual command patterns
- Excessive API calls
- Unauthorized file access

### Alerting Rules

Set up alerts for:
- Dangerous command attempts
- Budget threshold exceeded
- MCP server failures
- Rate limit violations

## Incident Response

### Security Incident Checklist

1. **Immediate Actions**:
   - [ ] Revoke compromised credentials
   - [ ] Disable affected MCP servers
   - [ ] Review audit logs
   - [ ] Isolate affected systems

2. **Investigation**:
   - [ ] Analyze command history
   - [ ] Review file modifications
   - [ ] Check for data exfiltration
   - [ ] Identify attack vector

3. **Remediation**:
   - [ ] Patch vulnerabilities
   - [ ] Update security rules
   - [ ] Rotate all credentials
   - [ ] Document lessons learned

## Security Updates

Stay current with security patches:

1. **Dependencies**: Regularly update Rust dependencies:
   ```bash
   cargo update
   cargo audit
   ```

2. **Base Images**: Update container base images monthly

3. **Security Advisories**: Monitor:
   - AWS Security Bulletins
   - Rust Security Advisories
   - GitHub Security Advisories

## Compliance

### Data Protection

- **Encryption at Rest**: Use encrypted storage
- **Encryption in Transit**: TLS 1.2+ for all connections
- **Data Retention**: Define and enforce retention policies
- **PII Handling**: Avoid processing personal information

### Audit Requirements

Maintain audit logs for:
- Minimum 90 days (hot storage)
- 1 year (cold storage/archive)
- Immutable storage for compliance

## Security Contact

Report security vulnerabilities to:
- Email: security@example.com
- GPG Key: [public key]

**Do not report security issues via public GitHub issues.**

## Additional Resources

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CIS Benchmarks](https://www.cisecurity.org/cis-benchmarks/)
- [AWS Security Best Practices](https://aws.amazon.com/security/best-practices/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
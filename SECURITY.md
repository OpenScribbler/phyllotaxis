# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please report it privately rather than opening a public issue.

**Email:** openscribbler.dev@pm.me

Include:
- A description of the vulnerability
- Steps to reproduce
- The potential impact

I'll acknowledge receipt within 48 hours and aim to provide a fix or mitigation within 7 days.

## Scope

Phyllotaxis parses user-provided OpenAPI spec files (YAML and JSON). Security concerns include:
- Malicious spec files causing unexpected behavior
- Path traversal via `$ref` resolution
- Resource exhaustion from deeply nested or circular schemas

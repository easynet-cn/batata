# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in Batata, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email: **124244236@qq.com**

Include the following in your report:

- Description of the vulnerability
- Steps to reproduce
- Affected version(s)
- Potential impact
- Suggested fix (if any)

## Response Timeline

- **Acknowledgment**: within 48 hours
- **Initial assessment**: within 7 days
- **Fix or mitigation**: depends on severity, typically within 30 days

## Security Measures

Batata implements the following security practices:

- **Constant-time comparison** for all token/secret authentication using the `subtle` crate (CWE-208 mitigation)
- **JWT-based authentication** with configurable secret keys
- **bcrypt password hashing**
- **Role-based access control (RBAC)** for API endpoints
- **Input validation** via the `validator` crate

## Scope

This policy applies to the official Batata repository at https://github.com/easynet-cn/batata.

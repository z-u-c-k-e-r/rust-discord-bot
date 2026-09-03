# Security policy

## Supported versions

The project is in foundation development. Security fixes are applied to the latest `main` branch until tagged releases begin.

## Reporting a vulnerability

Do not open a public issue for a vulnerability that could expose tokens, sessions, guild data or remote code execution.

Use GitHub's private vulnerability reporting feature for this repository when enabled. Include:

- affected commit or version
- impact
- reproduction steps
- required Discord permissions
- whether a Lua module is involved
- suggested mitigation, if known

Do not include real Discord tokens, OAuth2 secrets, cookies or personal data in the report.

## Scope

High-priority reports include:

- Lua sandbox escape
- token or credential disclosure
- OAuth2 state/session bypass
- cross-guild authorization bypass
- CSRF that changes guild settings
- SQL injection
- SSRF through media inputs
- permission or role-hierarchy bypass
- arbitrary file access or process execution
- persistent cross-site scripting in the dashboard

See [`docs/SECURITY.md`](docs/SECURITY.md) for the current security model.

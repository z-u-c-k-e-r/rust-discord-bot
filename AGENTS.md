# AGENTS.md

This repository is a security-sensitive Discord automation platform.

## Product rule

Guild behavior should be scriptable in Lua. Rust is the privileged host and must remain the only layer with direct Discord, database, voice, network, process and filesystem capabilities.

## Mandatory architecture rules

1. Do not pass secrets or infrastructure clients into Lua.
2. Do not execute arbitrary strings as shell commands.
3. Do not add unrestricted HTTP, SQL or filesystem Lua functions.
4. Every new Lua action needs validation, permission checks, hierarchy checks when relevant, auditing, tests and documentation.
5. Treat Discord IDs as strings at Lua, JSON and dashboard boundaries.
6. Authorize every dashboard guild route server-side.
7. Use HttpOnly cookies and CSRF protection for dashboard writes.
8. Disable uncontrolled mentions in generated messages.
9. Keep music URL input behind an HTTPS host allowlist.
10. Do not claim a feature is complete until it is connected end to end.

## Before changing code

Read:

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/LUA_API.md`
- `docs/SECURITY.md`
- `docs/FEATURE_MATRIX.md`

## Required checks

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Pull-request explanation

Every PR must state:

- the problem
- the chosen approach
- all affected files
- important code paths
- permissions and security consequences
- database consequences
- tests run and their results
- known risks
- how to explain the change in a technical interview

Do not merge generated changes that the repository owner cannot review and explain.

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

## Version policy

1. Read `docs/VERSION_POLICY.md` before adding or updating dependencies.
2. Verify current stable upstream releases on their official sources; do not copy versions from memory.
3. Keep `rust-toolchain.toml`, `Cargo.toml`, `Dockerfile`, `compose.yaml` and CI synchronized.
4. Use stable production releases only. Previews, release candidates and nightlies require a separate experimental branch.
5. Never downgrade a component merely to hide a migration error. Document and implement the migration.

## Before changing code

Read:

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/LUA_API.md`
- `docs/SECURITY.md`
- `docs/FEATURE_MATRIX.md`
- `docs/VERSION_POLICY.md`

## Required checks

```bash
bash scripts/check-latest-rust.sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
docker build --tag zuckerbot:local .
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

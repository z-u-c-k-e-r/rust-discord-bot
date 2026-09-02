# Dependency modernization gate

- Rust: newest stable exact toolchain, edition 2024.
- Crates: newest stable mutually compatible releases.
- Lua: newest stable language generation supported by the selected `mlua` release.
- Discord: current Discord API and newest stable Discord/voice libraries.
- Infrastructure: current stable PostgreSQL, Redis, container bases and CI actions.
- Verification: rustfmt, Clippy with `-D warnings`, tests, security audit and release build.
- Exception rule: an older component requires a reproduced blocker and documented migration task.

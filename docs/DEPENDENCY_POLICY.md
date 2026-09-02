# Dependency policy

ZuckerBot tracks the newest stable Rust toolchain, Rust edition, Discord API support, Lua runtime, Rust crates, database engines, container images and CI actions.

Every technology addition or upgrade must be verified against its primary release source. Production manifests use explicit versions and the application commit includes `Cargo.lock` for reproducible builds.

A component may remain below the newest stable release only when the newer release is demonstrably incompatible with the current platform or another required dependency. The exact blocker, reproduction command and migration follow-up must then be recorded in `DEPENDENCY_STATUS.md`.

Automated update checks do not bypass review. Every update must pass formatting, Clippy with warnings denied, the complete test suite, security auditing and a release build before merge.

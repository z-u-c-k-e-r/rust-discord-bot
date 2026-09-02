# Dependency policy

ZuckerBot tracks the newest stable Rust toolchain and Rust edition, together with the newest stable mutually compatible versions of the Discord integration, Lua runtime, web stack, persistence layer, database engines, container images and CI actions.

## Selection rules

1. Verify every addition or upgrade against a primary release source.
2. Pin the Rust toolchain explicitly and commit `Cargo.lock` for reproducible application builds.
3. Prefer the newest stable release, including a new major version, after the complete migration gate passes.
4. Keep an older release only when the newest release has a reproduced compatibility blocker. Select the newest version that does pass and record the blocker, reproduction command and migration follow-up in `DEPENDENCY_STATUS.md`.
5. Do not use wildcard dependency versions or silently retain a version merely because it worked previously.

## Required merge gate

Every dependency or platform update must pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

Security and supply-chain checks are mandatory in CI. Dependabot monitors Cargo, GitHub Actions and Docker dependencies; automated pull requests still require review and the full gate.

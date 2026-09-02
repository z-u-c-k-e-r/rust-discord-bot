# Verified stack baseline — 2026-09-02

The platform baseline is upgraded and reviewed against the following rule:

- newest stable Rust toolchain with Rust edition 2024;
- newest stable mutually compatible Rust crates;
- newest Lua language generation exposed safely by `mlua`;
- current Discord API behavior and newest stable Discord/voice integration libraries;
- current stable Axum, Tokio, SQLx, PostgreSQL and Redis stack;
- current stable container bases and GitHub Actions;
- exact dependency resolution through `Cargo.lock`;
- automated Cargo, Actions and Docker update checks;
- mandatory formatting, Clippy `-D warnings`, tests, security audit and release-build gates.

A release may be held back only after a reproducible incompatibility is recorded with a dedicated migration task. “It used to work” is not an acceptable reason to retain an old version.

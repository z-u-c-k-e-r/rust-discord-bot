# Latest stable stack rule

This project uses the newest stable Rust toolchain and Rust edition supported by stable Rust, together with the newest stable mutually compatible versions of the Discord library, Lua runtime, web stack, persistence layer, container images and CI actions.

Before a component is added or upgraded, its current stable release must be verified from a primary source. Exact application resolutions are committed in `Cargo.lock` and checked in CI.

Using an older release is allowed only when the newest stable release has a reproduced compatibility blocker. The selected fallback must be the newest version that passes the complete build and test gate, and the blocker must be documented with a migration follow-up.

# Development guide

## Toolchain

The repository pins Rust 1.98.0 through `rust-toolchain.toml`. CI verifies that this exact version matches Rust's official stable channel before compiling the project.

System packages commonly required on Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install --yes build-essential cmake libopus-dev pkg-config ffmpeg yt-dlp
```

The production container does not trust the distribution's potentially older `yt-dlp` package. It downloads the audited upstream release named in `Dockerfile` and verifies its SHA-256 digest before installation.

See [`VERSION_POLICY.md`](VERSION_POLICY.md) before changing Rust, crate, container, database or CI versions.

## Initial setup

```bash
cp .env.example .env
docker compose up -d postgres
cargo run
```

For fast command iteration, set `DISCORD_DEV_GUILD_ID`. Guild commands update immediately; global command propagation is intentionally not used during normal development.

## Quality gate

Run before every pull request:

```bash
bash scripts/check-latest-rust.sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
docker build --tag zuckerbot:local .
```

CI runs the same Rust checks and builds the production container independently.

## Adding a Lua module

1. Add a `.lua` file under `scripts/`.
2. Return a valid module table.
3. Use unique module and command IDs.
4. Define a configuration schema, even if it is an empty object.
5. Return only documented actions.
6. Add a test that loads the module and exercises its important branches.
7. Document required Discord intents and permissions.
8. Add the capability to the feature matrix.

The process refuses to start if any module has an invalid manifest or duplicate command.

## Adding a Rust action

A new action is a security-sensitive change.

Required implementation areas:

- `src/lua/model.rs`: serialized contract and local input validation
- `src/discord/executor.rs`: permission, hierarchy and execution logic
- storage: audit record or durable state when relevant
- `docs/LUA_API.md`: public contract
- tests: valid, invalid, unauthorized and boundary cases
- dashboard/schema support when configurable

Never expose a raw Serenity `Context`, HTTP client, SQL pool or filesystem handle to Lua.

## Adding a Gateway event

1. Add the Serenity handler.
2. Convert the event into a stable `LuaEventContext`.
3. Expose Discord IDs as strings.
4. Include only data needed by modules.
5. Document privileged intents.
6. Define whether event actions run as automation or on behalf of an actor.
7. Add redaction and retention decisions for logged data.

Avoid serializing complete Serenity objects. That would couple Lua modules to library internals and expose data unintentionally.

## Database migrations

Add ordered SQL files under `migrations/`.

Rules:

- migrations must be forward-only once released
- use explicit indexes for query paths
- avoid storing JavaScript-unsafe snowflakes as numeric values
- include retention or cleanup strategy for high-volume tables
- separate runtime and migration database roles in production

The supplied Compose stack currently targets PostgreSQL 18.6. PostgreSQL 18 changed the official container's volume layout, so persistent data is mounted at `/var/lib/postgresql`, not the pre-18 `/var/lib/postgresql/data` location.

## Dashboard frontend

The foundation frontend uses plain HTML, CSS and JavaScript so the Rust binary has no Node build requirement.

Rules:

- use `textContent`, not `innerHTML`, for untrusted values
- require CSRF on writes
- authorize every guild path on the server
- do not trust a guild ID supplied by the browser
- do not store OAuth access tokens in the browser or application session
- keep controls usable on desktop and mobile
- use accessible labels and focus states

A future framework migration requires a measurable benefit and must preserve the server-side authorization model.

## Voice development

The Songbird input resolver expects `yt-dlp` at runtime for supported web sources.

Do not add source-specific bypasses. Source adapters must:

- comply with the service's terms
- validate schemes and hosts
- use timeouts and size limits
- avoid internal network access
- expose operational metrics
- fail without blocking a shard

## Branch and pull-request conventions

Recommended branch names:

```text
feat/moderation-cases
feat/workflow-engine
fix/oauth-session-expiry
docs/lua-actions
```

Recommended commit messages:

```text
feat(lua): add scheduled-message action
fix(auth): consume OAuth state atomically
test(moderation): cover equal-role hierarchy
docs(security): document media worker isolation
```

A pull request should explain:

- user-facing behavior
- architecture impact
- security impact
- database changes
- test evidence
- deployment or rollback requirements

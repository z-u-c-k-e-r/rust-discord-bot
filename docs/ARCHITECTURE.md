# Architecture

## Design goals

ZuckerBot is built around five requirements:

1. Server owners can change behavior in Lua without recompiling Rust.
2. Untrusted or faulty Lua must not receive infrastructure credentials or unrestricted capabilities.
3. Every privileged action must pass Rust-side validation and Discord authorization checks.
4. The same runtime must support one development guild and a future sharded multi-tenant deployment.
5. Dashboard configuration and Discord execution must share one source of truth.

## Components

### Discord host

`src/discord` owns the Serenity client and Songbird integration.

Responsibilities:

- connect to the Discord Gateway
- register application commands generated from Lua manifests
- convert Discord interactions and events into stable Lua contexts
- enforce interaction response rules
- check actor and application permissions
- check Discord role hierarchy for moderation and role actions
- execute allowlisted actions
- persist audit events

The Discord layer does not contain guild-specific command behavior.

### Lua engine

`src/lua` loads all `.lua` files under `SCRIPTS_DIR` and builds an immutable registry. A reload is built separately and swapped only after every module passes validation, preventing a partially loaded state.

Each execution creates a fresh Lua state. This trades some raw throughput for strong isolation between invocations and predictable cleanup. Later releases can add a bounded VM pool after benchmarks and isolation tests exist.

Controls applied to every state:

- safe Lua standard-library subset
- no `io`, `os`, `package`, `require`, `dofile`, `loadfile` or `debug`
- fixed memory ceiling
- instruction-count hook
- maximum number of returned actions
- schema validation of manifests
- validation of every returned action

Lua returns data. It does not call Discord APIs directly.

### Action boundary

`LuaAction` is the privileged contract between Lua and Rust. Adding a new action requires all of the following:

1. a serializable action model
2. input validation
3. actor permission requirements
4. bot permission requirements
5. hierarchy or ownership checks when relevant
6. audit behavior
7. tests
8. documentation

This makes the allowed attack surface explicit and reviewable.

### Storage

`src/storage` currently provides:

- PostgreSQL in persistent deployments
- an in-memory fallback for local development
- guild module settings
- append-only audit events

The initial migration also reserves tables for moderation cases, module key/value state and scheduled jobs. Runtime APIs for those tables are added only when their authorization and lifecycle contracts are implemented.

### Dashboard

`src/web` is an Axum application with a dependency-free frontend.

Authentication flow:

1. browser requests `/auth/discord`
2. the server creates a one-time OAuth2 state value
3. Discord redirects back with a code and state
4. the server validates and consumes the state
5. the server exchanges the code and fetches the user plus guild list
6. only owned guilds or guilds with Manage Guild / Administrator are retained
7. the Discord access token is discarded
8. an opaque HttpOnly application session is issued

Write requests require the session CSRF token.

## Command flow

```text
InteractionCreate
  -> resolve command to module
  -> load per-guild enable flag and configuration
  -> convert Discord values to string-safe Lua context
  -> execute sandboxed on_command
  -> deserialize and validate actions
  -> send initial interaction response
  -> authorize each privileged action
  -> check target hierarchy
  -> execute Discord/voice/storage operation
  -> write audit event
```

Discord snowflakes are exposed to Lua and the dashboard as strings. This avoids precision loss in JavaScript and keeps future storage migrations predictable.

## Event flow

```text
Gateway event
  -> find subscribed modules
  -> load module settings
  -> execute sandboxed on_event
  -> validate actions
  -> execute as configured automation
  -> write audit event
```

Automated event actions do not inherit permissions from the user who caused an event. Enabling such a module is an administrative decision; the bot's own permissions and hierarchy remain the final Discord-side boundary.

## Scaling path

The foundation runs the dashboard and Discord client in one process. The interfaces are intentionally separable.

Planned scale-out:

- stateless dashboard replicas
- Redis-backed sessions and distributed locks
- shard workers grouped by Discord shard ID
- PostgreSQL as configuration and durable state
- NATS or Redis Streams for internal events
- dedicated scheduler workers
- dedicated media nodes for voice
- OpenTelemetry traces, metrics and structured logs
- rolling Lua module registry versions

No distributed component should be introduced before load tests demonstrate a concrete need.

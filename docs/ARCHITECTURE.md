# ZuckerBot architecture

## Goal

ZuckerBot is intended to become a multi-tenant Discord automation platform, not a single hard-coded bot. Rust owns every privileged capability. Lua modules describe commands and request a narrow allow-list of actions. The web control plane stores configuration per guild.

## Current milestone: 0.1 foundation

```text
Discord Gateway / Interactions
            |
            v
     Serenity event layer
            |
            v
  command manifest resolver  <---- Lua module manifests
            |
            v
      sandboxed Lua run
            |
            v
   typed LuaAction allow-list
            |
            v
 Rust permission checks + Discord/Songbird APIs

Browser ----> Axum dashboard API ----> atomic guild JSON storage
                         |
                         +----> atomic Lua reload
```

The first milestone deliberately stays in one process so it can be run and understood locally. The public Rust interfaces already separate the Discord data plane, Lua runtime, storage and web control plane, which allows them to be split into services later.

## Trust boundaries

### Rust is trusted

Rust owns:

- the Discord bot token and HTTP client;
- Gateway and interaction handling;
- slash-command registration;
- permission checks before every privileged action;
- guild configuration storage;
- voice connections and the music queue;
- dashboard authentication;
- Lua memory, instruction and wall-clock limits.

### Lua is untrusted

A Lua module receives a serializable command context and may only return supported `LuaAction` values. It does **not** receive:

- filesystem access;
- process or shell execution;
- sockets or an arbitrary HTTP client;
- environment variables;
- Discord tokens;
- raw Serenity objects;
- direct database access.

Each invocation gets a fresh Lua state. This prevents globals and mutable state from leaking between users or guilds. Persistent state will be exposed later through a typed, quota-controlled key-value API instead of unrestricted database access.

## Command lifecycle

1. At startup every `scripts/*.lua` file is evaluated in the sandbox.
2. Its `manifest` is deserialized and validated.
3. Duplicate module and command names reject the complete reload.
4. Serenity synchronizes global application commands with Discord.
5. On an interaction, Rust resolves the owning module and verifies that it is enabled for the guild.
6. Rust verifies permissions declared in the command manifest.
7. Lua receives `CommandContext` and returns an array of typed actions.
8. Rust validates action-specific permissions again and executes the action.
9. Errors are logged and returned as an ephemeral interaction response.

The duplicated permission check is intentional. A script author cannot bypass moderation permissions by declaring a harmless command and returning a `kick` or `ban` action.

## Configuration model

Guild configuration is persisted as JSON in `DATA_DIR`:

```json
{
  "enabled_modules": ["core", "fun", "moderation", "music"],
  "module_config": {
    "music": { "default_volume": 0.75 },
    "moderation": { "log_channel_id": "123456789012345678" }
  }
}
```

`enabled_modules: null` means all installed modules are enabled. Writes use a temporary file followed by an atomic rename. The process also keeps a read-through cache.

This storage is appropriate for one instance. The production architecture will replace it with PostgreSQL, Redis and an append-only audit stream while retaining the same `Storage` boundary.

## Dashboard security

Milestone 0.1 uses a long `DASHBOARD_TOKEN` sent as a Bearer token. It is suitable for local development or a private owner-only deployment behind HTTPS. It is not the final multi-user authorization model.

The next dashboard milestone will add:

- Discord OAuth2 authorization-code flow;
- encrypted server-side sessions;
- CSRF protection;
- guild selection restricted to users with `MANAGE_GUILD` or `ADMINISTRATOR`;
- role-based staff access;
- audit events for every configuration change;
- rate limits and secret rotation.

## Target production topology

```text
                         +---------------------+
Discord Gateway -------> | shard workers       |
                         +----------+----------+
                                    |
                                    v
                         +---------------------+
                         | event/command bus    | <---- Redis Streams / NATS
                         +---+-----------+-----+
                             |           |
                    +--------+--+   +----+----------------+
                    | Lua workers|   | moderation workers  |
                    +--------+--+   +----+----------------+
                             |           |
                             +-----+-----+
                                   v
                         +---------------------+
                         | PostgreSQL + audit  |
                         +---------------------+

Browser -> CDN/reverse proxy -> dashboard API -> OAuth/RBAC -> configuration service
Voice commands -------------------------------> regional Songbird voice nodes
```

## Scaling rules

- Guild ownership is partitioned by Discord shard ID.
- Commands are idempotent where Discord can retry delivery.
- Long jobs are deferred and executed through a durable queue.
- Rate-limit state is shared by workers.
- Music is isolated from Gateway workers so media failures cannot disconnect the bot.
- Every configuration mutation and moderation action receives an immutable audit record.
- Lua modules have versioned schemas and migrations.

## Observability target

The production platform should expose:

- structured JSON logs with interaction, guild, user and module correlation IDs;
- Prometheus metrics for Gateway latency, Discord rate limits, command duration, Lua failures, queue depth and voice health;
- OpenTelemetry traces across web, queue and worker boundaries;
- per-module error budgets and automatic circuit breakers;
- owner-facing incident and health pages.

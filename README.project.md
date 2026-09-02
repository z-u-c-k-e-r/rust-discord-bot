# ZuckerBot

**A Lua-scriptable Discord automation platform written in Rust.**

ZuckerBot is being built as a safer, extensible alternative to large all-in-one bots such as MEE6. Rust owns Discord access, permissions, storage, the dashboard and voice. Lua modules declare slash commands and may request only a typed allow-list of actions.

> **Project status: 0.1 foundation.** The repository now contains a functional architecture and first modules; it does not yet claim full feature parity with mature commercial bots. The complete target scope is tracked in [`docs/CAPABILITY_MATRIX.md`](docs/CAPABILITY_MATRIX.md) and [`docs/ROADMAP.md`](docs/ROADMAP.md).

## What is implemented

- Rust 2024 application using Serenity, Songbird, mlua and Axum.
- Slash commands generated from Lua manifests.
- Fresh Lua sandbox for every command invocation.
- Lua memory, instruction and wall-clock limits.
- Atomic module reload: one invalid module rejects the complete replacement.
- Duplicate command detection and manifest validation.
- Per-guild module enable/disable configuration.
- Per-module arbitrary JSON configuration available as `ctx.module_config`.
- Responsive owner dashboard and authenticated JSON API.
- Action-level permission checks in Rust for moderation.
- Basic Songbird voice connection and music queue controls.
- Atomic JSON persistence for a single-instance deployment.
- Docker, Docker Compose and GitHub Actions CI.

### Included Lua modules and commands

| Module | Commands |
|---|---|
| `core` | `/ping`, `/bot`, `/help` |
| `fun` | `/coinflip`, `/roll`, `/meme` |
| `moderation` | `/kick`, `/ban` |
| `music` | `/join`, `/play`, `/pause`, `/resume`, `/skip`, `/stop`, `/leave` |

## Security model

Lua is treated as untrusted extension code.

Lua never receives:

- the Discord token;
- unrestricted HTTP or sockets;
- filesystem or shell access;
- environment variables;
- a database connection;
- raw Serenity objects.

A module receives a serializable command context and returns typed actions such as `reply`, `kick` or `music_play`. Rust verifies module state, Discord permissions and action-specific permissions before calling Discord.

This means a malicious or broken script cannot turn a harmless slash command into an unauthorized ban simply by changing its manifest.

Read the complete design in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requirements

- current stable Rust toolchain;
- a Discord application and bot token;
- `ffmpeg` and `yt-dlp` for music playback;
- `libopus` development libraries when compiling Songbird on Linux;
- HTTPS/reverse proxy for any dashboard exposed outside a private network.

## Discord application setup

1. Create an application in the Discord Developer Portal.
2. Create its bot user and copy the token.
3. Invite it with the `bot` and `applications.commands` OAuth2 scopes.
4. Grant only permissions needed by enabled modules. The starter moderation module needs Kick Members and Ban Members; music needs Connect and Speak.
5. Put the token in `.env`. Never commit it.

Global application-command updates can take time to propagate. Development guild commands will be added in a later milestone for instant test registration.

## Local start

```bash
cp .env.example .env
```

Set at least:

```dotenv
DISCORD_TOKEN=your-discord-bot-token
DASHBOARD_TOKEN=generate-a-long-random-secret-at-least-24-characters
```

Then run:

```bash
cargo run
```

The default dashboard address is:

```text
http://localhost:8080
```

The dashboard asks for `DASHBOARD_TOKEN` and a Discord Guild ID. Version 0.1 is owner-only. Discord OAuth2, manageable-guild selection, sessions, CSRF protection and staff RBAC are the immediate next milestone.

## Docker

```bash
cp .env.example .env
# edit .env
docker compose up --build -d
```

Persistent guild configuration is mounted from `./data`. Lua files are mounted read-only from `./scripts`, so they can be edited and then reloaded from the dashboard.

## Writing a Lua command

Create a file under `scripts/`:

```lua
return {
  manifest = {
    name = "hello",
    version = "1.0.0",
    description = "Example module",
    commands = {
      {
        name = "hello",
        description = "Greets the current user"
      }
    }
  },

  handle = function(ctx)
    return {
      {
        type = "reply",
        content = "Cześć, " .. ctx.username .. "!",
        ephemeral = false
      }
    }
  end
}
```

Reload modules through the dashboard or:

```bash
curl -X POST \
  -H "Authorization: Bearer $DASHBOARD_TOKEN" \
  http://localhost:8080/api/reload
```

The complete manifest, context and action contract is documented in [`docs/LUA_API.md`](docs/LUA_API.md).

## Dashboard API

All `/api/*` routes require:

```http
Authorization: Bearer <DASHBOARD_TOKEN>
```

Current routes:

| Method | Route | Purpose |
|---|---|---|
| `GET` | `/health` | Process health check; no authentication required. |
| `GET` | `/api/modules` | Installed Lua modules and commands. |
| `POST` | `/api/reload` | Atomically reload all Lua modules. |
| `GET` | `/api/guilds/{guild_id}/config` | Read guild module configuration. |
| `PUT` | `/api/guilds/{guild_id}/config` | Validate and atomically save configuration. |

## Repository layout

```text
src/
  config.rs      environment and runtime limits
  discord.rs     Serenity interactions and Rust action executor
  lua.rs         sandbox, manifests, reload and execution
  model.rs       serialized command/action/config contracts
  storage.rs     atomic per-guild persistence
  web.rs         Axum dashboard API
scripts/         first-party Lua modules
web/             dashboard frontend
docs/            architecture, Lua SDK, capability matrix and roadmap
```

## Product direction

The planned platform covers moderation/AutoMod, onboarding and roles, tickets/forms, XP and economy, music/voice, notifications, events, giveaways, analytics, integrations and a signed Lua extension ecosystem. Features are delivered in security-first milestones rather than pretending that every category is already complete.

Key documents:

- [`docs/CAPABILITY_MATRIX.md`](docs/CAPABILITY_MATRIX.md) — everything the target bot/dashboard should support.
- [`docs/ROADMAP.md`](docs/ROADMAP.md) — delivery phases from 0.1 to the extension marketplace.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — trust boundaries and production topology.
- [`docs/LUA_API.md`](docs/LUA_API.md) — current scripting contract.

## Contributing

Keep privileged behavior in Rust and expose it through a narrow typed action. Do not give Lua arbitrary operating-system, database or network access. Every new moderation action must have a second permission check in the Rust executor and tests for denied access.

## License

MIT — see [`LICENSE`](LICENSE).

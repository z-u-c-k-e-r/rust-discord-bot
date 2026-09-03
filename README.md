# ZuckerBot

A secure, Lua-scriptable Discord automation platform with a high-performance Rust core, PostgreSQL storage, voice support and a first-party web dashboard.

ZuckerBot is designed as an extensible alternative to closed all-in-one bots. Server behavior lives in versioned Lua modules; Discord connectivity, authorization, storage, rate-sensitive operations and privileged actions remain behind a validated Rust API.

> **Current status:** foundation release. The repository already contains the runtime, dashboard, OAuth2 login, PostgreSQL migrations, Discord command registration, voice queue integration and example Lua modules. The complete product scope is tracked in [`docs/FEATURE_MATRIX.md`](docs/FEATURE_MATRIX.md), while the audited Discord API surface is recorded in [`docs/DISCORD_PLATFORM_COVERAGE.md`](docs/DISCORD_PLATFORM_COVERAGE.md). [`docs/VERSION_POLICY.md`](docs/VERSION_POLICY.md) defines the latest-stable dependency policy.

## Why Rust plus Lua?

Rust owns the trust boundary:

- Discord Gateway and HTTP clients
- permission and role-hierarchy checks
- OAuth2 and browser sessions
- database access and audit records
- voice connections and media source validation
- Lua memory, instruction and action limits

Lua owns server behavior:

- slash-command manifests
- command installation and interaction contexts
- command options and choices
- reactions to Discord events
- per-guild configuration schemas
- declarative actions returned to the Rust host

A Lua script never receives the Discord bot token, raw database credentials, unrestricted network access or direct filesystem access.

## Included foundation

- Discord slash commands generated from Lua manifests
- current global-command `integration_types`, `contexts` and NSFW declarations
- controlled migration from the deprecated `dm_permission` manifest field
- event dispatch for `message_create` and `guild_member_add`
- sandboxed Lua 5.4 with memory and global instruction limits
- validated action API for replies, messages, moderation, roles, purge, music, progression, persistent scheduling and audit events
- action-level Discord permission checks
- bot, moderator and target role-hierarchy checks
- Songbird voice connection and queue controls
- HTTPS media-host allowlist to reduce SSRF exposure
- Discord OAuth2 dashboard login
- per-guild module enable/disable and JSON configuration
- HttpOnly sessions, OAuth2 state validation and CSRF protection
- PostgreSQL migrations with an in-memory development fallback
- audit records for privileged actions, scheduler transitions and dashboard changes
- lease-based PostgreSQL scheduler with retries, pause/resume and an in-memory test fallback
- Docker Compose development deployment
- CI formatting, linting, tests, release build and production-container build

Bundled Lua modules:

| Module | Commands or events |
| --- | --- |
| `core.lua` | `/ping`, `/about` |
| `fun.lua` | `/meme`, `/roll`, `/choose`, `/eightball` |
| `moderation.lua` | `/moderate` with timeout, kick, ban and purge |
| `roles.lua` | `/role` add/remove |
| `music.lua` | `/music` play, pause, resume, skip, stop, queue and leave |
| `welcome.lua` | configurable `guild_member_add` welcome automation |
| `automod.lua` | multi-rule message safety and anti-abuse engine |
| `progression.lua` | XP, levels, coins, daily streaks, reputation and leaderboards |
| `scheduler.lua` | persistent reminders and recurring server messages |
| `community.lua` | suggestions, reports, feedback, applications and bug reports |
| `utility.lua` | calculations, conversions, timestamps and Discord utilities |
| `games.lua` | social games, dice, teams and cosmetic drops |
| `staff_tools.lua` | announcements, broadcasts, alerts, rules and audit notes |
| `server_info.lua` | rules, FAQ, links, support, staff and schedule |
| `join_guard.lua` | account-age and suspicious-join safety signals |

## Architecture

```text
Discord Gateway / Interactions
             |
             v
      Rust Discord host
             |
     +-------+--------+
     |                |
     v                v
Lua sandbox      Action validator
     |                |
     +-------> allowed actions
                      |
          +-----------+-----------+
          |           |           |
          v           v           v
      Discord API  PostgreSQL   Songbird
          ^
          |
Discord OAuth2 web dashboard
```

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for trust boundaries and request flows.

## Local development

### Requirements

- Rust 1.98.0; CI verifies that the pinned toolchain matches Rust's current stable channel
- PostgreSQL 18.6 through the supplied Compose stack, unless using the volatile in-memory fallback
- a Discord application with a bot user
- `ffmpeg`, `yt-dlp` and Opus runtime support for music

### Discord application

1. Create a Discord application and bot.
2. Enable the **Server Members Intent** and **Message Content Intent** while the bundled event modules are in use.
3. Add the OAuth2 redirect URI used by `DISCORD_OAUTH_REDIRECT_URL`.
4. Invite the bot with the `bot` and `applications.commands` scopes.
5. Grant only the permissions needed by enabled modules. Do not use Administrator in production unless there is a documented reason.

### Configure

```bash
cp .env.example .env
```

Fill in at least:

```dotenv
DISCORD_TOKEN=...
DISCORD_APPLICATION_ID=...
DISCORD_CLIENT_ID=...
DISCORD_CLIENT_SECRET=...
DISCORD_OAUTH_REDIRECT_URL=http://127.0.0.1:8080/auth/discord/callback
DISCORD_DEV_GUILD_ID=...
```

Using `DISCORD_DEV_GUILD_ID` registers commands on one development server, where updates appear immediately. Remove it before production to register global commands with their declared installation and interaction contexts.

### Run with Docker Compose

```bash
docker compose up --build
```

Open the dashboard at `http://127.0.0.1:8080`.

### Run directly

```bash
cargo run
```

Without `DATABASE_URL`, configuration is kept only in memory and is lost when the process exits.

## A minimal Lua module

```lua
return {
    manifest = {
        id = "hello",
        name = "Hello",
        version = "1.0.0",
        description = "A minimal ZuckerBot module.",
        category = "example",
        default_enabled = true,
        commands = {
            {
                name = "hello",
                description = "Greets the invoking user.",
                integration_types = { "guild", "user" },
                contexts = { "guild", "bot_dm", "private_channel" },
                nsfw = false,
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {},
        },
    },

    on_command = function(command, ctx)
        if command == "hello" then
            return {
                {
                    type = "reply",
                    content = "Hello, " .. zuckerbot.escape_mentions(ctx.user_name) .. ".",
                    ephemeral = false,
                },
            }
        end
        return {}
    end,
}
```

The full manifest, context and action contract is documented in [`docs/LUA_API.md`](docs/LUA_API.md).

## Repository layout

```text
src/
  discord/       Discord events, command registration and action execution
  lua/           sandbox, manifests, contexts and action contract
  storage/       PostgreSQL and in-memory stores
  web/           OAuth2 dashboard and API
scripts/         first-party Lua modules
web/static/      dependency-free dashboard frontend
migrations/      PostgreSQL schema
docs/            architecture, security, API and product scope
tests/           Lua runtime and safety tests
```

## Development commands

```bash
bash scripts/check-latest-rust.sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
docker build --tag zuckerbot:local .
```

## Music and external services

The music module is a host integration, not a license to copy protected content or bypass a platform's access rules. Deployments must use sources they are permitted to access and must comply with the source platform's terms, applicable copyright law and Discord's policies. URL input is restricted to HTTPS hosts configured through `MUSIC_ALLOWED_HOSTS`.

## Security

Read [`SECURITY.md`](SECURITY.md) and [`docs/SECURITY.md`](docs/SECURITY.md) before exposing a deployment to untrusted users. The Lua sandbox is a defense layer, not a substitute for reviewing third-party modules.

## Contributing

Review [`AGENTS.md`](AGENTS.md) and [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md). Changes to the Lua action contract must include validation, authorization, audit behavior and tests in the same pull request.

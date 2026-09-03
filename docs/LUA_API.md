# Lua module API

API version: `1`

Every file ending in `.lua` below `SCRIPTS_DIR` must return one module table.

```lua
return {
    manifest = { ... },
    on_command = function(command, ctx) return { ...actions } end,
    on_event = function(event, ctx) return { ...actions } end,
}
```

A module may implement commands, events or both. Unknown commands and events should return an empty array `{}`.

## Manifest

Required fields:

| Field | Type | Rules |
| --- | --- | --- |
| `id` | string | 1–64 lowercase ASCII letters, digits, `_` or `-` |
| `name` | string | 1–64 characters |
| `version` | string | module version; semantic versioning is recommended |
| `description` | string | 1–200 characters |
| `category` | string | 1–32 characters |

Optional fields:

| Field | Type | Default |
| --- | --- | --- |
| `default_enabled` | boolean | `true` |
| `commands` | array | empty |
| `events` | array of strings | empty |
| `config_schema` | table | empty object |

`config_schema` is exposed to the dashboard. The foundation stores and displays JSON configuration; full JSON Schema form generation and validation are tracked in the feature roadmap.

## Command definition

```lua
{
    name = "example",
    description = "Runs an example command.",
    integration_types = { "guild" },
    contexts = { "guild" },
    nsfw = false,
    default_member_permissions = "32",
    options = {
        {
            type = "string",
            name = "mode",
            description = "Operation mode.",
            required = true,
            autocomplete = false,
            choices = {
                { name = "Fast", value = "fast" },
                { name = "Safe", value = "safe" },
            },
            min_length = 1,
            max_length = 30,
        },
    },
}
```

### Installation and interaction contexts

Current Discord commands use two separate declarations:

- `integration_types` controls how the app was installed. Values: `guild`, `user`.
- `contexts` controls where the command may run. Values: `guild`, `bot_dm`, `private_channel`.

Both default to `{ "guild" }` in ZuckerBot. A `private_channel` context requires the `user` installation type. Empty arrays and duplicate values are rejected. These fields apply only to global command registration; the development-guild registration path omits them because Discord does not accept global-only context fields on guild commands.

The historic `dm_permission` field is deprecated by Discord. API version 1 accepts it only as a migration input:

- `dm_permission = false` maps to `{ "guild" }`;
- `dm_permission = true` maps to `{ "guild", "bot_dm" }`;
- it cannot be combined with `contexts`;
- ZuckerBot never sends `dm_permission` back to Discord;
- it is scheduled for removal in Lua API version 2.

Set `nsfw = true` only for commands that are intentionally age-restricted.

Supported option types:

- `string`
- `integer`
- `number`
- `boolean`
- `user`
- `channel`
- `role`
- `mentionable`
- `attachment`
- `subcommand`
- `subcommand_group`

Subcommands and subcommand groups contain nested `options`. Discord limits each level to 25 entries. The runtime rejects duplicate command names and refuses command registration if the total number of global commands exceeds 100.

`default_member_permissions` is a Discord permission bitset encoded as a string. Privileged actions are checked again at execution time; command visibility is not treated as authorization.

## Command context

```lua
ctx = {
    guild_id = "123" or nil,
    channel_id = "456",
    user_id = "789",
    user_name = "Display name",
    member_roles = { "111", "222" },
    member_permissions = "1099511627776",
    locale = "pl",
    options = {
        action = "timeout",
        user = "999",
    },
    config = {
        default_timeout_seconds = 600,
    },
}
```

All Discord snowflakes are strings at Lua and JSON boundaries. Nested subcommand options are represented as nested tables.

## Event context

```lua
ctx = {
    name = "message_create",
    guild_id = "123" or nil,
    channel_id = "456" or nil,
    actor_id = "789" or nil,
    data = { ...event-specific fields... },
    config = { ...guild module configuration... },
}
```

Implemented events:

### `message_create`

`data` contains `message_id`, `content`, author data, mentions and attachment metadata.

### `guild_member_add`

`data` contains user data, roles and `joined_at`.

The stable event envelope lets future versions add events without exposing Serenity internals directly to Lua.

## Actions

A handler returns an array of action tables. A single execution may return at most 25 actions.

### Reply

```lua
{
    type = "reply",
    content = "Done.",
    ephemeral = true,
}
```

For commands, the first reply becomes the interaction response. Additional replies become follow-up messages. For events, a reply uses the event channel and fails if the event has no channel context.

### Send message

```lua
{
    type = "send_message",
    channel_id = "123", -- optional when a channel exists in the context
    content = "Message text",
}
```

Generated messages use an empty allowed-mentions policy. Text such as `@everyone` cannot trigger an uncontrolled mass mention.

### Delete message

```lua
{
    type = "delete_message",
    channel_id = "123",
    message_id = "456",
}
```

Required Discord permission: Manage Messages.

### Timeout member

```lua
{
    type = "timeout_member",
    user_id = "123",
    seconds = 600,
    reason = "Repeated spam",
}
```

`seconds` must be between 1 and 2,419,200. Required permission: Moderate Members. Rust checks moderator and bot hierarchy before execution.

### Kick member

```lua
{
    type = "kick_member",
    user_id = "123",
    reason = "Rule violation",
}
```

Required permission: Kick Members. Rust checks moderator and bot hierarchy.

### Ban member

```lua
{
    type = "ban_member",
    user_id = "123",
    delete_message_days = 0,
    reason = "Raid account",
}
```

`delete_message_days` must be between 0 and 7. Required permission: Ban Members. Rust checks moderator and bot hierarchy.

### Add or remove role

```lua
{
    type = "add_role", -- or remove_role
    user_id = "123",
    role_id = "456",
    reason = "Verified member",
}
```

Required permission: Manage Roles. The target role must be below the bot and invoking moderator in the hierarchy.

### Purge

```lua
{
    type = "purge",
    channel_id = "123", -- optional in a command channel
    amount = 25,
}
```

`amount` must be between 1 and 100. Required permission: Manage Messages.

### Moderation cases and warnings

Lua can create a durable moderation case without receiving database access:

```lua
{
    type = "create_moderation_case",
    target_user_id = "123",
    case_type = "warning",
    reason = "Repeated spam",
    points = 2,
    expires_in_seconds = 7776000,
    metadata = {
        source = "manual_warning",
    },
    escalation_rules = {
        {
            threshold_points = 3,
            action = "timeout",
            duration_seconds = 3600,
        },
        {
            threshold_points = 7,
            action = "kick",
        },
        {
            threshold_points = 10,
            action = "ban",
            delete_message_days = 1,
        },
    },
}
```

The Rust capability broker:

- validates the target and moderator hierarchy;
- stores the case in PostgreSQL or the development memory store;
- counts points from open, non-expired cases;
- applies the highest matching Lua-declared escalation rule;
- checks the moderator and bot permission required by that escalation;
- records case creation and escalation in the audit log.

Limits: case type 1–32 lowercase ASCII characters, reason 1–512 characters,
0–1000 points, expiry up to 365 days, up to 32 metadata keys/16 KiB and up to
10 strictly increasing escalation rules.

Read cases:

```lua
{
    type = "list_moderation_cases",
    target_user_id = "123",
    limit = 10,
    include_resolved = false,
}
```

Resolve an open case:

```lua
{
    type = "resolve_moderation_case",
    case_id = 42,
    resolution = "Appeal accepted",
}
```

All three actions require Moderate Members for interactive commands. Automated
event modules can create cases through the same broker; the bot identity is then
stored as the moderator. Dashboard endpoints expose the same storage lifecycle:

```text
GET /api/guilds/{guild_id}/moderation/users/{target_user_id}/cases
PUT /api/guilds/{guild_id}/moderation/cases/{case_id}/resolve
```

### Music

```lua
{
    type = "music",
    operation = "play",
    query = "artist and title",
}
```

Operations: `play`, `pause`, `resume`, `skip`, `stop`, `queue`, `leave`.

A URL must use HTTPS and match `MUSIC_ALLOWED_HOSTS`. Plain text is treated as a search query. Music actions require a guild and user voice context.

### Audit

```lua
{
    type = "audit",
    event = "custom_event",
    data = {
        key = "value",
    },
}
```

The event name must contain 1–64 bytes. Audit data is stored as JSON.

## Built-in helper API

The global `zuckerbot` table contains safe pure helpers:

```lua
zuckerbot.api_version
zuckerbot.escape_mentions(text)
zuckerbot.truncate(text, max_characters)
zuckerbot.unix_time()
```

`escape_mentions` inserts a zero-width separator after `@`. Use it whenever untrusted display names or user-provided strings are included in output.

## Sandbox

Unavailable globals:

- `collectgarbage`
- `coroutine`
- `debug`
- `dofile`
- `io`
- `load`
- `loadfile`
- `os`
- `package`
- `pcall`
- `require`
- `xpcall`

Each execution uses:

- a fresh Lua state;
- a configurable memory limit;
- a global instruction hook covering new Lua threads;
- a maximum of 25 returned actions;
- post-deserialization action validation.

Configuration environment variables:

```dotenv
LUA_MEMORY_LIMIT_BYTES=8388608
LUA_INSTRUCTION_LIMIT=500000
LUA_HOOK_GRANULARITY=1000
```

The sandbox intentionally has no direct network, database, process, Discord-client or filesystem API. New capabilities must be modeled as narrow declarative actions and reviewed at the Rust boundary.

## Module development checklist

1. Choose a globally unique module ID and command names.
2. Keep the manifest free of side effects.
3. Use `integration_types` and `contexts`, not `dm_permission`.
4. Treat every context field as untrusted input.
5. Escape user-controlled text before returning it.
6. Return a user-visible reply for commands.
7. Use the smallest privileged action possible.
8. Add configuration schema metadata.
9. Add runtime tests for success and failure paths.
10. Document Discord permissions and intents.
11. Review the module before enabling it on production guilds.

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
    dm_permission = false,
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

Subcommands and subcommand groups contain nested `options`. Discord limits each level to 25 entries. The runtime rejects duplicate command names and refuses to start if the total number of global commands exceeds 100.

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

All Discord snowflakes should be treated as strings. Nested subcommand options are represented as nested tables.

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

`data` contains:

- `message_id`
- `content`
- `author.id`
- `author.name`
- `author.global_name`
- `mentions`
- `attachments`

### `guild_member_add`

`data` contains:

- `user.id`
- `user.name`
- `user.global_name`
- `user.bot`
- `roles`
- `joined_at`

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

### Music

```lua
{
    type = "music",
    operation = "play",
    query = "artist and title",
}
```

Operations:

- `play`
- `pause`
- `resume`
- `skip`
- `stop`
- `queue`
- `leave`

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

`escape_mentions` inserts a zero-width separator after `@`. It should be used whenever untrusted display names or user-provided strings are included in output.

## Sandbox

Unavailable globals:

- `debug`
- `dofile`
- `io`
- `loadfile`
- `os`
- `package`
- `require`

Each execution uses:

- a fresh Lua state
- a configurable memory limit
- a configurable instruction limit
- a maximum of 25 returned actions
- post-deserialization action validation

Configuration environment variables:

```dotenv
LUA_MEMORY_LIMIT_BYTES=8388608
LUA_INSTRUCTION_LIMIT=500000
LUA_HOOK_GRANULARITY=1000
```

The sandbox intentionally has no direct network, database, process or filesystem API. New capabilities must be modeled as narrow declarative actions and reviewed at the Rust boundary.

## Module development checklist

1. Choose a globally unique module ID and command names.
2. Keep the manifest free of side effects.
3. Treat every context field as untrusted input.
4. Escape user-controlled text before returning it.
5. Return a user-visible reply for commands.
6. Use the smallest privileged action possible.
7. Add configuration schema metadata.
8. Add runtime tests for failure paths.
9. Document Discord permissions and intents.
10. Review the module before enabling it on production guilds.

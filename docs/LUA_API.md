# Lua module API

Every file in `scripts/` must return one table containing `manifest` and `handle`.

```lua
return {
  manifest = {
    name = "example",
    version = "1.0.0",
    description = "Example module",
    commands = {
      {
        name = "hello",
        description = "Greets a user",
        dm_permission = true,
        required_permissions = {},
        options = {
          {
            name = "user",
            description = "User to greet",
            kind = "user",
            required = false
          }
        }
      }
    }
  },

  handle = function(ctx)
    local target = ctx.options.user or ctx.user_id
    return {
      {
        type = "reply",
        content = "Hello <@" .. target .. ">",
        ephemeral = false
      }
    }
  end
}
```

## Manifest

### Module fields

| Field | Type | Required | Notes |
|---|---:|---:|---|
| `name` | string | yes | Lowercase ASCII, digits, `_` and `-`, maximum 32 characters. |
| `version` | string | no | Defaults to `0.1.0`. |
| `description` | string | no | Maximum 200 characters. |
| `commands` | array | yes | At least one command. Command names must be globally unique. |

### Command fields

| Field | Type | Required | Notes |
|---|---:|---:|---|
| `name` | string | yes | Discord slash-command name. |
| `description` | string | yes | 1–100 characters. |
| `dm_permission` | boolean | no | Defaults to `true`. Set `false` for guild-only actions. |
| `nsfw` | boolean | no | Marks the application command as age-restricted. |
| `required_permissions` | string[] | no | Permissions checked by Discord and again by Rust. |
| `options` | array | no | Slash-command arguments. |

Supported permission names:

- `administrator`
- `manage_guild`
- `manage_messages`
- `kick_members`
- `ban_members`
- `moderate_members`

### Option fields

| Field | Type | Required | Notes |
|---|---:|---:|---|
| `name` | string | yes | Lowercase identifier. |
| `description` | string | yes | 1–100 characters. |
| `kind` | string | yes | One of the option kinds below. |
| `required` | boolean | no | Defaults to `false`. |
| `min_integer` / `max_integer` | integer | no | Bounds for integer options. |
| `min_length` / `max_length` | integer | no | Bounds for string options. |

Supported option kinds:

- `string`
- `integer`
- `number`
- `boolean`
- `user`
- `channel`
- `role`
- `mentionable`
- `attachment`

## Command context

`handle(ctx)` receives:

```lua
ctx = {
  command = "kick",
  guild_id = "123456789012345678", -- nil in a DM
  channel_id = "123456789012345678",
  user_id = "123456789012345678",
  username = "Zucker",
  options = {
    user = "987654321098765432",
    reason = "spam"
  },
  module_config = {
    log_channel_id = "111111111111111111"
  }
}
```

Discord snowflake IDs are strings. Do not convert the complete value with `tonumber`; a Lua number cannot precisely represent every 64-bit Discord ID.

## Actions

A handler returns an array. Actions run in order.

### Reply to an interaction

```lua
{
  type = "reply",
  content = "Visible response",
  ephemeral = true
}
```

### Send a channel message

```lua
{
  type = "send_message",
  content = "Message"
}
```

### Kick

```lua
{
  type = "kick",
  user_id = "123456789012345678",
  reason = "Audit log reason"
}
```

Rust requires the invoker to have `KICK_MEMBERS` even if the manifest was configured incorrectly.

### Ban

```lua
{
  type = "ban",
  user_id = "123456789012345678",
  reason = "Audit log reason",
  delete_message_seconds = 3600
}
```

`delete_message_seconds` must stay within Discord's accepted range. Rust requires `BAN_MEMBERS`.

### Voice and music

```lua
{ type = "voice_join" }
{ type = "voice_leave" }
{ type = "music_play", query = "URL or search phrase" }
{ type = "music_pause" }
{ type = "music_resume" }
{ type = "music_skip" }
{ type = "music_stop" }
```

`music_play` automatically joins the command author's voice channel when the bot is not connected.

## Runtime limits

A fresh Lua state is created for every command. Limits are controlled through environment variables:

- `LUA_MEMORY_BYTES`
- `LUA_INSTRUCTION_LIMIT`
- `LUA_TIMEOUT_MS`

The safe standard library does not expose unrestricted filesystem, process, environment or socket access. A module cannot call Discord directly. It can only return actions understood by the Rust enum.

## Reload behavior

`POST /api/reload` parses all modules into a temporary registry. The live registry is replaced only when every module and every command manifest passes validation. Duplicate command names reject the reload.

## Planned API extensions

The allow-list will grow through versioned capabilities rather than exposing raw Rust or Discord objects. Planned groups include:

- embeds, files, buttons, selects and modals;
- roles, channels, threads, forums and scheduled events;
- warnings, timeouts, message purge and case management;
- typed key-value persistence with per-module quotas;
- delayed jobs and cron schedules;
- approved HTTP integrations with domain allow-lists;
- economy, XP and achievements;
- tickets, forms and transcripts;
- analytics events and audit logging;
- music queue metadata, filters and playlists.

return {
    manifest = {
        id = "welcome",
        name = "Welcome and Goodbye",
        version = "0.1.0",
        description = "Konfigurowalne powitania nowych członków sterowane zdarzeniami Lua.",
        category = "automation",
        default_enabled = false,
        events = {
            "guild_member_add",
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            required = { "channel_id" },
            properties = {
                channel_id = {
                    type = "string",
                    title = "ID kanału powitalnego",
                },
                message = {
                    type = "string",
                    title = "Treść powitania",
                    default = "Witaj {user} na serwerze.",
                    maxLength = 1800,
                },
            },
        },
    },

    on_event = function(event, ctx)
        if event ~= "guild_member_add" then
            return {}
        end
        if not ctx.config.channel_id or ctx.config.channel_id == "" then
            return {}
        end

        local template = ctx.config.message or "Witaj {user} na serwerze."
        local user_id = tostring(ctx.data.user.id)
        local safe_mention = "<@" .. user_id .. ">"
        local safe_name = zuckerbot.escape_mentions(tostring(ctx.data.user.global_name or ctx.data.user.name or ""))
        local content = string.gsub(template, "{user}", function() return safe_mention end)
        content = string.gsub(content, "{username}", function() return safe_name end)

        return {
            {
                type = "send_message",
                channel_id = ctx.config.channel_id,
                content = zuckerbot.truncate(content, 2000),
            },
            {
                type = "audit",
                event = "welcome_sent",
                data = {
                    user_id = user_id,
                    channel_id = ctx.config.channel_id,
                },
            },
        }
    end,
}

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
        local user_id = ctx.data.user.id
        local content = string.gsub(template, "{user}", "<@" .. user_id .. ">")
        content = string.gsub(content, "{username}", zuckerbot.escape_mentions(ctx.data.user.global_name or ctx.data.user.name))

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

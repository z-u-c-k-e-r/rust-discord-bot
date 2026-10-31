return {
    manifest = {
        id = "welcome",
        name = "Welcome and Goodbye",
        version = "1.0.0",
        description = "Konfigurowalne powitania, pożegnania, automatyczne role i wiadomości prywatne dla nowych członków.",
        category = "automation",
        default_enabled = false,
        events = {
            "guild_member_add",
            "guild_member_remove",
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                channel_id = {
                    type = "string",
                    title = "ID kanału powitalnego",
                },
                message = {
                    type = "string",
                    title = "Treść powitania",
                    default = "Witaj {user} na serwerze {server}! Jesteś naszym {member_count}. członkiem.",
                    maxLength = 1800,
                },
                dm_welcome = {
                    type = "boolean",
                    title = "Wysyłaj powitanie w wiadomości prywatnej (DM)",
                    default = false,
                },
                dm_message = {
                    type = "string",
                    title = "Treść powitania w DM",
                    default = "Witaj na serwerze {server}! Zapoznaj się z regulaminem i miłego pobytu.",
                    maxLength = 1800,
                },
                auto_roles = {
                    type = "array",
                    title = "Role automatycznie nadawane po dołączeniu",
                    items = { type = "string" },
                    maxItems = 10,
                },
                send_goodbye = {
                    type = "boolean",
                    title = "Wysyłaj wiadomości pożegnalne",
                    default = false,
                },
                goodbye_channel_id = {
                    type = "string",
                    title = "ID kanału pożegnalnego",
                },
                goodbye_message = {
                    type = "string",
                    title = "Treść pożegnania",
                    default = "Żegnaj {username}. Będziemy tęsknić!",
                    maxLength = 1800,
                },
            },
        },
    },

    on_event = function(event, ctx)
        if event == "guild_member_add" then
            local actions = {}
            local user = ctx.data.user or {}
            local user_id = tostring(user.id or "")
            if user_id == "" then return {} end

            local server_name = tostring(ctx.data.guild_name or "naszym serwerze")
            local member_count = tostring(ctx.data.member_count or "")
            local safe_mention = "<@" .. user_id .. ">"
            local safe_name = zuckerbot.escape_mentions(tostring(user.global_name or user.name or ""))

            local function format_text(template)
                local content = string.gsub(template, "{user}", function() return safe_mention end)
                content = string.gsub(content, "{username}", function() return safe_name end)
                content = string.gsub(content, "{server}", function() return server_name end)
                content = string.gsub(content, "{member_count}", function() return member_count end)
                return zuckerbot.truncate(content, 2000)
            end

            -- Kanał powitalny
            if ctx.config.channel_id and ctx.config.channel_id ~= "" then
                local template = ctx.config.message or "Witaj {user} na serwerze {server}."
                table.insert(actions, {
                    type = "send_message",
                    channel_id = ctx.config.channel_id,
                    content = format_text(template),
                })
                table.insert(actions, {
                    type = "audit",
                    event = "welcome_sent",
                    data = {
                        user_id = user_id,
                        channel_id = ctx.config.channel_id,
                    },
                })
            end

            -- Wiadomość prywatna (DM)
            if ctx.config.dm_welcome == true and not user.bot then
                local dm_template = ctx.config.dm_message or "Witaj na serwerze {server}! Zapoznaj się z regulaminem i miłego pobytu."
                table.insert(actions, {
                    type = "send_direct_message",
                    user_id = user_id,
                    content = format_text(dm_template),
                })
            end

            -- Automatyczne role dla nowego członka
            if type(ctx.config.auto_roles) == "table" then
                for _, role_id in ipairs(ctx.config.auto_roles) do
                    local rid = tostring(role_id or "")
                    if rid ~= "" then
                        table.insert(actions, {
                            type = "add_role",
                            user_id = user_id,
                            role_id = rid,
                            reason = "Automatyczna rola powitalna dla nowego członka",
                        })
                    end
                end
            end

            return actions
        end

        if event == "guild_member_remove" then
            if ctx.config.send_goodbye ~= true then return {} end

            local target_channel = ctx.config.goodbye_channel_id
            if not target_channel or target_channel == "" then
                target_channel = ctx.config.channel_id
            end
            if not target_channel or target_channel == "" then return {} end

            local user = ctx.data.user or {}
            local user_id = tostring(user.id or "")
            local server_name = tostring(ctx.data.guild_name or "naszym serwerze")
            local member_count = tostring(ctx.data.member_count or "")
            local safe_name = zuckerbot.escape_mentions(tostring(user.global_name or user.name or "użytkowniku"))

            local template = ctx.config.goodbye_message or "Żegnaj {username}. Będziemy tęsknić!"
            local content = string.gsub(template, "{username}", function() return safe_name end)
            content = string.gsub(content, "{server}", function() return server_name end)
            content = string.gsub(content, "{member_count}", function() return member_count end)
            content = zuckerbot.truncate(content, 2000)

            return {
                {
                    type = "send_message",
                    channel_id = target_channel,
                    content = content,
                },
                {
                    type = "audit",
                    event = "goodbye_sent",
                    data = {
                        user_id = user_id,
                        channel_id = target_channel,
                    },
                },
            }
        end

        return {}
    end,
}

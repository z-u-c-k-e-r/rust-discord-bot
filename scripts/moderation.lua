local function missing(message)
    return {
        {
            type = "reply",
            content = message,
            ephemeral = true,
        },
    }
end

return {
    manifest = {
        id = "moderation",
        name = "Moderation",
        version = "0.2.0",
        description = "Bezpieczne akcje moderacyjne z kontrolą uprawnień, hierarchii i audytem.",
        category = "safety",
        default_enabled = true,
        commands = {
            {
                name = "moderate",
                description = "Wykonuje timeout, kick, ban albo czyszczenie wiadomości.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Akcja moderacyjna.",
                        required = true,
                        choices = {
                            { name = "Timeout", value = "timeout" },
                            { name = "Kick", value = "kick" },
                            { name = "Ban", value = "ban" },
                            { name = "Purge", value = "purge" },
                        },
                    },
                    {
                        type = "user",
                        name = "user",
                        description = "Użytkownik dla timeout, kick albo ban.",
                        required = false,
                    },
                    {
                        type = "integer",
                        name = "seconds",
                        description = "Czas timeoutu w sekundach.",
                        required = false,
                        min_value = 1,
                        max_value = 2419200,
                    },
                    {
                        type = "integer",
                        name = "amount",
                        description = "Liczba wiadomości do usunięcia.",
                        required = false,
                        min_value = 1,
                        max_value = 100,
                    },
                    {
                        type = "string",
                        name = "reason",
                        description = "Powód widoczny w audycie Discorda.",
                        required = false,
                        max_length = 512,
                    },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                default_timeout_seconds = {
                    type = "integer",
                    minimum = 1,
                    maximum = 2419200,
                    default = 600,
                },
            },
        },
    },

    on_command = function(command, ctx)
        if command ~= "moderate" then
            return {}
        end

        local action = ctx.options.action
        local user_id = ctx.options.user
        local reason = ctx.options.reason or "Brak powodu"
        local accepted = {
            type = "reply",
            content = "Akcja moderacyjna została przekazana do bezpiecznego wykonania.",
            ephemeral = true,
        }

        if action == "purge" then
            return {
                accepted,
                {
                    type = "purge",
                    amount = tonumber(ctx.options.amount) or 10,
                },
            }
        end

        if not user_id then
            return missing("Dla tej akcji musisz wskazać użytkownika.")
        end

        if action == "timeout" then
            local default_seconds = tonumber(ctx.config.default_timeout_seconds) or 600
            return {
                accepted,
                {
                    type = "timeout_member",
                    user_id = user_id,
                    seconds = tonumber(ctx.options.seconds) or default_seconds,
                    reason = reason,
                },
            }
        end

        if action == "kick" then
            return {
                accepted,
                {
                    type = "kick_member",
                    user_id = user_id,
                    reason = reason,
                },
            }
        end

        if action == "ban" then
            return {
                accepted,
                {
                    type = "ban_member",
                    user_id = user_id,
                    delete_message_days = 0,
                    reason = reason,
                },
            }
        end

        return missing("Nieznana akcja moderacyjna.")
    end,
}

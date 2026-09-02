return {
    manifest = {
        id = "core",
        name = "Core",
        version = "0.1.0",
        description = "Podstawowe komendy diagnostyczne i informacje o platformie.",
        category = "platform",
        default_enabled = true,
        commands = {
            {
                name = "ping",
                description = "Sprawdza, czy bot i silnik Lua odpowiadają.",
                dm_permission = true,
            },
            {
                name = "about",
                description = "Pokazuje informacje o ZuckerBocie.",
                dm_permission = true,
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {},
        },
    },

    on_command = function(command, ctx)
        if command == "ping" then
            return {
                {
                    type = "reply",
                    content = "Pong. Rust core i sandbox Lua działają poprawnie.",
                    ephemeral = true,
                },
                {
                    type = "audit",
                    event = "ping",
                    data = {
                        user_id = ctx.user_id,
                        locale = ctx.locale,
                    },
                },
            }
        end

        if command == "about" then
            return {
                {
                    type = "reply",
                    content = "ZuckerBot to modułowa platforma Discord: bezpieczny rdzeń Rust, logika Lua, panel WWW, PostgreSQL, audyt i obsługa głosu.",
                    ephemeral = true,
                },
            }
        end

        return {}
    end,
}

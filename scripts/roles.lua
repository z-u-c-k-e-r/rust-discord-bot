return {
    manifest = {
        id = "roles",
        name = "Role Management",
        version = "0.2.0",
        description = "Kontrolowane nadawanie i odbieranie ról z ochroną hierarchii.",
        category = "administration",
        default_enabled = true,
        commands = {
            {
                name = "role",
                description = "Nadaje albo odbiera rolę użytkownikowi.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Dodaj albo usuń rolę.",
                        required = true,
                        choices = {
                            { name = "Add", value = "add" },
                            { name = "Remove", value = "remove" },
                        },
                    },
                    {
                        type = "user",
                        name = "user",
                        description = "Użytkownik, którego role zmieniasz.",
                        required = true,
                    },
                    {
                        type = "role",
                        name = "role",
                        description = "Rola do dodania albo usunięcia.",
                        required = true,
                    },
                    {
                        type = "string",
                        name = "reason",
                        description = "Powód zmiany.",
                        required = false,
                        max_length = 512,
                    },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {},
        },
    },

    on_command = function(command, ctx)
        if command ~= "role" then
            return {}
        end

        local action_type = ctx.options.action == "add" and "add_role" or "remove_role"
        return {
            {
                type = "reply",
                content = "Zmiana roli została przekazana do wykonania.",
                ephemeral = true,
            },
            {
                type = action_type,
                user_id = ctx.options.user,
                role_id = ctx.options.role,
                reason = ctx.options.reason or "Zmiana przez ZuckerBot",
            },
        }
    end,
}

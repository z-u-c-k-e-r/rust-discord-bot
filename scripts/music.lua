return {
    manifest = {
        id = "music",
        name = "Music",
        version = "0.1.0",
        description = "Odtwarzanie, kolejka i sterowanie głosem przez izolowane akcje Lua.",
        category = "voice",
        default_enabled = true,
        commands = {
            {
                name = "music",
                description = "Steruje odtwarzaniem muzyki na kanale głosowym.",
                dm_permission = false,
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Operacja na odtwarzaczu.",
                        required = true,
                        choices = {
                            { name = "Play", value = "play" },
                            { name = "Pause", value = "pause" },
                            { name = "Resume", value = "resume" },
                            { name = "Skip", value = "skip" },
                            { name = "Stop", value = "stop" },
                            { name = "Queue", value = "queue" },
                            { name = "Leave", value = "leave" },
                        },
                    },
                    {
                        type = "string",
                        name = "query",
                        description = "Tytuł albo dozwolony adres HTTPS.",
                        required = false,
                        max_length = 500,
                    },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                announce_tracks = {
                    type = "boolean",
                    default = true,
                },
                default_volume = {
                    type = "number",
                    minimum = 0.0,
                    maximum = 1.0,
                    default = 0.5,
                },
            },
        },
    },

    on_command = function(command, ctx)
        if command ~= "music" then
            return {}
        end

        local operation = ctx.options.action
        if operation == "play" and (not ctx.options.query or ctx.options.query == "") then
            return {
                {
                    type = "reply",
                    content = "Dla akcji play podaj tytuł albo dozwolony adres HTTPS.",
                    ephemeral = true,
                },
            }
        end

        return {
            {
                type = "reply",
                content = "Polecenie muzyczne zostało przyjęte.",
                ephemeral = true,
            },
            {
                type = "music",
                operation = operation,
                query = ctx.options.query,
            },
        }
    end,
}

local ADMINISTRATOR = 8
local MANAGE_GUILD = 32

local function has_manage_guild(ctx)
    local permissions = math.tointeger(tonumber(ctx.member_permissions or "0")) or 0
    return (permissions & ADMINISTRATOR) == ADMINISTRATOR
        or (permissions & MANAGE_GUILD) == MANAGE_GUILD
end

local function reply(content)
    return {
        {
            type = "reply",
            content = content,
            ephemeral = true,
        },
    }
end

local function scheduler(operation)
    return {
        {
            type = "scheduler",
            operation = operation,
        },
    }
end

local function limits(config, staff)
    local minimum_delay = math.tointeger(config.minimum_delay_seconds or 30) or 30
    local maximum_days = math.tointeger(config.maximum_delay_days or 365) or 365
    local maximum_delay = maximum_days * 86400
    local maximum_jobs
    if staff then
        maximum_jobs = math.tointeger(config.max_staff_jobs or 100) or 100
    else
        maximum_jobs = math.tointeger(config.max_user_jobs or 10) or 10
    end
    return minimum_delay, maximum_delay, maximum_jobs
end

local function control_operation(action, job_id)
    if action == "cancel" then
        return { type = "cancel", job_id = job_id }
    end
    if action == "pause" then
        return { type = "pause", job_id = job_id }
    end
    if action == "resume" then
        return { type = "resume", job_id = job_id }
    end
    return nil
end

return {
    manifest = {
        id = "scheduler",
        name = "Scheduler and Reminders",
        version = "1.0.0",
        description = "Trwałe przypomnienia użytkowników oraz jednorazowe i cykliczne wiadomości serwera.",
        category = "automation",
        default_enabled = true,
        commands = {
            {
                name = "remind",
                description = "Tworzy trwałe przypomnienie na tym kanale.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "when",
                        description = "Np. 15m, 1d 2h, Unix timestamp albo RFC 3339.",
                        required = true,
                        min_length = 2,
                        max_length = 128,
                    },
                    {
                        type = "string",
                        name = "message",
                        description = "Treść przypomnienia.",
                        required = true,
                        min_length = 1,
                        max_length = 1800,
                    },
                    {
                        type = "boolean",
                        name = "mention",
                        description = "Czy bot ma oznaczyć autora przy wysłaniu.",
                        required = false,
                    },
                },
            },
            {
                name = "reminders",
                description = "Wyświetla twoje aktywne przypomnienia.",
                integration_types = { "guild" },
                contexts = { "guild" },
            },
            {
                name = "remindcontrol",
                description = "Anuluje, wstrzymuje albo wznawia twoje przypomnienie.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Operacja do wykonania.",
                        required = true,
                        choices = {
                            { name = "Anuluj", value = "cancel" },
                            { name = "Wstrzymaj", value = "pause" },
                            { name = "Wznów", value = "resume" },
                        },
                    },
                    {
                        type = "string",
                        name = "id",
                        description = "Pełny identyfikator UUID zadania.",
                        required = true,
                        min_length = 36,
                        max_length = 36,
                    },
                },
            },
            {
                name = "schedulemessage",
                description = "Planuje jednorazową albo cykliczną wiadomość serwera.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = "32",
                options = {
                    {
                        type = "string",
                        name = "when",
                        description = "Np. 15m, 1d 2h, Unix timestamp albo RFC 3339.",
                        required = true,
                        min_length = 2,
                        max_length = 128,
                    },
                    {
                        type = "string",
                        name = "message",
                        description = "Treść zaplanowanej wiadomości.",
                        required = true,
                        min_length = 1,
                        max_length = 1800,
                    },
                    {
                        type = "channel",
                        name = "channel",
                        description = "Kanał docelowy należący do tego serwera.",
                        required = true,
                    },
                    {
                        type = "string",
                        name = "repeat",
                        description = "Opcjonalny interwał, np. 1h, 1d albo 1w.",
                        required = false,
                        min_length = 2,
                        max_length = 64,
                    },
                    {
                        type = "integer",
                        name = "runs",
                        description = "Łączna liczba wykonań; brak oznacza cykl bez końca.",
                        required = false,
                        min_value = 2,
                        max_value = 10000,
                    },
                    {
                        type = "boolean",
                        name = "mention_creator",
                        description = "Czy oznaczyć autora harmonogramu przy wysłaniu.",
                        required = false,
                    },
                },
            },
            {
                name = "schedules",
                description = "Wyświetla aktywne zadania całego serwera.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = "32",
            },
            {
                name = "schedulecontrol",
                description = "Zarządza dowolnym zadaniem harmonogramu serwera.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = "32",
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Operacja do wykonania.",
                        required = true,
                        choices = {
                            { name = "Anuluj", value = "cancel" },
                            { name = "Wstrzymaj", value = "pause" },
                            { name = "Wznów", value = "resume" },
                        },
                    },
                    {
                        type = "string",
                        name = "id",
                        description = "Pełny identyfikator UUID zadania.",
                        required = true,
                        min_length = 36,
                        max_length = 36,
                    },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                user_reminders_enabled = {
                    type = "boolean",
                    default = true,
                },
                max_user_jobs = {
                    type = "integer",
                    minimum = 1,
                    maximum = 50,
                    default = 10,
                },
                max_staff_jobs = {
                    type = "integer",
                    minimum = 1,
                    maximum = 500,
                    default = 100,
                },
                minimum_delay_seconds = {
                    type = "integer",
                    minimum = 10,
                    maximum = 3600,
                    default = 30,
                },
                maximum_delay_days = {
                    type = "integer",
                    minimum = 1,
                    maximum = 3650,
                    default = 365,
                },
                allow_user_mentions = {
                    type = "boolean",
                    default = true,
                },
                default_mention_creator = {
                    type = "boolean",
                    default = false,
                },
            },
        },
    },

    on_command = function(command, ctx)
        if command == "remind" then
            if ctx.config.user_reminders_enabled == false then
                return reply("Przypomnienia użytkowników są wyłączone na tym serwerze.")
            end

            local minimum_delay, maximum_delay, maximum_jobs = limits(ctx.config, false)
            local mention_creator = ctx.options.mention == true
                and ctx.config.allow_user_mentions ~= false
            return scheduler({
                type = "create",
                when = ctx.options.when,
                content = ctx.options.message,
                mention_creator = mention_creator,
                max_jobs = maximum_jobs,
                minimum_delay_seconds = minimum_delay,
                maximum_delay_seconds = maximum_delay,
            })
        end

        if command == "reminders" then
            return scheduler({
                type = "list",
                scope = "mine",
                limit = 15,
            })
        end

        if command == "remindcontrol" then
            local operation = control_operation(ctx.options.action, ctx.options.id)
            if not operation then return reply("Nieznana operacja.") end
            return scheduler(operation)
        end

        if command == "schedulemessage" then
            if not has_manage_guild(ctx) then
                return reply("Ta komenda wymaga uprawnienia Zarządzanie serwerem.")
            end
            if ctx.options.runs and not ctx.options["repeat"] then
                return reply("Liczba wykonań wymaga podania interwału repeat.")
            end

            local minimum_delay, maximum_delay, maximum_jobs = limits(ctx.config, true)
            local mention_creator = ctx.options.mention_creator
            if mention_creator == nil then
                mention_creator = ctx.config.default_mention_creator == true
            end
            return scheduler({
                type = "create",
                when = ctx.options.when,
                content = ctx.options.message,
                channel_id = ctx.options.channel,
                ["repeat"] = ctx.options["repeat"],
                repeat_count = math.tointeger(ctx.options.runs),
                mention_creator = mention_creator,
                max_jobs = maximum_jobs,
                minimum_delay_seconds = minimum_delay,
                maximum_delay_seconds = maximum_delay,
            })
        end

        if command == "schedules" then
            if not has_manage_guild(ctx) then
                return reply("Ta komenda wymaga uprawnienia Zarządzanie serwerem.")
            end
            return scheduler({
                type = "list",
                scope = "all",
                limit = 20,
            })
        end

        if command == "schedulecontrol" then
            if not has_manage_guild(ctx) then
                return reply("Ta komenda wymaga uprawnienia Zarządzanie serwerem.")
            end
            local operation = control_operation(ctx.options.action, ctx.options.id)
            if not operation then return reply("Nieznana operacja.") end
            return scheduler(operation)
        end

        return {}
    end,
}

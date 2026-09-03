local ADMINISTRATOR = 8
local MANAGE_CHANNELS = 16
local MANAGE_GUILD = 32

local function reply(content)
    return {
        {
            type = "reply",
            content = content,
            ephemeral = true,
        },
    }
end

local function ticket(operation)
    return {
        {
            type = "ticket",
            operation = operation,
        },
    }
end

local function bounded_integer(value, fallback, minimum, maximum)
    local parsed = math.tointeger(value)
    if not parsed then return fallback end
    if parsed < minimum then return minimum end
    if parsed > maximum then return maximum end
    return parsed
end

local function optional_id(value)
    if type(value) == "string" and value:match("^%d+$") then
        return value
    end
    return nil
end

local function string_array(value)
    local result = {}
    if type(value) ~= "table" then return result end
    for _, item in ipairs(value) do
        if type(item) == "string" and item:match("^%d+$") then
            result[#result + 1] = item
        end
    end
    return result
end

local function has_permission(ctx, permission)
    local permissions = math.tointeger(tonumber(ctx.member_permissions or "0")) or 0
    return (permissions & ADMINISTRATOR) == ADMINISTRATOR
        or (permissions & permission) == permission
end

local function has_support_role(ctx, support_role_ids)
    local expected = {}
    for _, role_id in ipairs(support_role_ids) do
        expected[role_id] = true
    end
    for _, role_id in ipairs(ctx.member_roles or {}) do
        if expected[role_id] then return true end
    end
    return false
end

local function is_staff(ctx, support_role_ids)
    return has_permission(ctx, MANAGE_CHANNELS)
        or has_permission(ctx, MANAGE_GUILD)
        or has_support_role(ctx, support_role_ids)
end

local function normalize_identifier(value, fallback)
    value = tostring(value or fallback or "")
    value = value:lower():gsub("[^a-z0-9_-]", "-"):gsub("%-+", "-")
    value = value:gsub("^[-_]+", ""):gsub("[-_]+$", "")
    if value == "" then return fallback end
    return value
end

local function value_in_array(value, values)
    if type(values) ~= "table" or #values == 0 then return true end
    for _, configured in ipairs(values) do
        if value == configured then return true end
    end
    return false
end

local function support_roles(config)
    return string_array(config.support_role_ids)
end

local function open_policy(config)
    return {
        open_category_id = optional_id(config.open_category_id) or "",
        archive_category_id = optional_id(config.archive_category_id),
        support_role_ids = support_roles(config),
        log_channel_id = optional_id(config.log_channel_id),
        max_open_per_user = bounded_integer(config.max_open_per_user, 3, 1, 25),
        channel_name_prefix = normalize_identifier(config.channel_name_prefix, "ticket"),
        welcome_message = type(config.welcome_message) == "string"
            and config.welcome_message
            or nil,
    }
end

local function close_policy(config)
    return {
        support_role_ids = support_roles(config),
        archive_category_id = optional_id(config.archive_category_id),
        log_channel_id = optional_id(config.log_channel_id),
        creator_can_close = config.creator_can_close ~= false,
        generate_transcript = config.generate_transcript_on_close ~= false,
        transcript_max_messages = bounded_integer(
            config.transcript_max_messages,
            500,
            1,
            1000
        ),
    }
end

local function require_categories(config)
    if not optional_id(config.open_category_id) then
        return "Administrator musi najpierw ustawić `open_category_id` modułu tickets w panelu WWW."
    end
    return nil
end

return {
    manifest = {
        id = "tickets",
        name = "Tickets and Support",
        version = "1.0.0",
        description = "Prywatne tickety, kolejki wsparcia, claim, uczestnicy, priorytety, archiwizacja i transkrypcje.",
        category = "support",
        default_enabled = true,
        commands = {
            {
                name = "ticket",
                description = "Tworzy i obsługuje prywatne tickety wsparcia.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "subcommand",
                        name = "open",
                        description = "Otwiera prywatny ticket wsparcia.",
                        options = {
                            {
                                type = "string",
                                name = "subject",
                                description = "Krótki temat zgłoszenia.",
                                required = true,
                                min_length = 1,
                                max_length = 100,
                            },
                            {
                                type = "string",
                                name = "description",
                                description = "Dokładny opis problemu lub prośby.",
                                required = true,
                                min_length = 1,
                                max_length = 1800,
                            },
                            {
                                type = "string",
                                name = "queue",
                                description = "Kolejka, np. support, billing, report albo appeal.",
                                required = false,
                                min_length = 1,
                                max_length = 32,
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "list",
                        description = "Pokazuje aktywne tickety użytkownika lub serwera.",
                        options = {
                            {
                                type = "string",
                                name = "scope",
                                description = "Zakres listy.",
                                required = false,
                                choices = {
                                    { name = "Moje tickety", value = "mine" },
                                    { name = "Wszystkie tickety", value = "all" },
                                },
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "info",
                        description = "Pokazuje stan bieżącego ticketu i ostatnie zdarzenia.",
                    },
                    {
                        type = "subcommand",
                        name = "claim",
                        description = "Przejmuje bieżący ticket jako członek zespołu wsparcia.",
                    },
                    {
                        type = "subcommand",
                        name = "unclaim",
                        description = "Zwalnia przejęty ticket.",
                    },
                    {
                        type = "subcommand",
                        name = "close",
                        description = "Zamyka, archiwizuje i opcjonalnie zapisuje transkrypcję.",
                        options = {
                            {
                                type = "string",
                                name = "reason",
                                description = "Powód zamknięcia ticketu.",
                                required = false,
                                min_length = 1,
                                max_length = 512,
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "reopen",
                        description = "Ponownie otwiera zamknięty ticket.",
                    },
                    {
                        type = "subcommand",
                        name = "add",
                        description = "Dodaje użytkownika jako uczestnika bieżącego ticketu.",
                        options = {
                            {
                                type = "user",
                                name = "user",
                                description = "Użytkownik, który otrzyma dostęp.",
                                required = true,
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "remove",
                        description = "Usuwa uczestnika z bieżącego ticketu.",
                        options = {
                            {
                                type = "user",
                                name = "user",
                                description = "Użytkownik, któremu odebrać dostęp.",
                                required = true,
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "rename",
                        description = "Zmienia nazwę kanału bieżącego ticketu.",
                        options = {
                            {
                                type = "string",
                                name = "name",
                                description = "Nowa nazwa kanału.",
                                required = true,
                                min_length = 2,
                                max_length = 100,
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "priority",
                        description = "Ustawia priorytet bieżącego ticketu.",
                        options = {
                            {
                                type = "string",
                                name = "level",
                                description = "Nowy priorytet.",
                                required = true,
                                choices = {
                                    { name = "Niski", value = "low" },
                                    { name = "Normalny", value = "normal" },
                                    { name = "Wysoki", value = "high" },
                                    { name = "Pilny", value = "urgent" },
                                },
                            },
                        },
                    },
                    {
                        type = "subcommand",
                        name = "transcript",
                        description = "Tworzy i zapisuje transkrypcję bieżącego ticketu.",
                        options = {
                            {
                                type = "integer",
                                name = "messages",
                                description = "Maksymalna liczba najnowszych wiadomości.",
                                required = false,
                                min_value = 1,
                                max_value = 1000,
                            },
                        },
                    },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                open_category_id = {
                    type = "string",
                    pattern = "^[0-9]+$",
                    default = "",
                    description = "ID kategorii dla otwartych ticketów.",
                },
                archive_category_id = {
                    type = { "string", "null" },
                    pattern = "^[0-9]+$",
                    default = nil,
                    description = "Opcjonalne ID kategorii archiwum.",
                },
                log_channel_id = {
                    type = { "string", "null" },
                    pattern = "^[0-9]+$",
                    default = nil,
                    description = "Opcjonalny kanał audytu i transkrypcji.",
                },
                support_role_ids = {
                    type = "array",
                    maxItems = 20,
                    uniqueItems = true,
                    items = {
                        type = "string",
                        pattern = "^[0-9]+$",
                    },
                    default = {},
                },
                allowed_queues = {
                    type = "array",
                    maxItems = 25,
                    uniqueItems = true,
                    items = {
                        type = "string",
                        pattern = "^[a-z0-9_-]{1,32}$",
                    },
                    default = { "support", "billing", "report", "appeal" },
                },
                default_queue = {
                    type = "string",
                    pattern = "^[a-z0-9_-]{1,32}$",
                    default = "support",
                },
                max_open_per_user = {
                    type = "integer",
                    minimum = 1,
                    maximum = 25,
                    default = 3,
                },
                channel_name_prefix = {
                    type = "string",
                    pattern = "^[a-z0-9_-]{1,24}$",
                    default = "ticket",
                },
                welcome_message = {
                    type = { "string", "null" },
                    maxLength = 1500,
                    default = nil,
                },
                creator_can_close = {
                    type = "boolean",
                    default = true,
                },
                creator_can_manage_participants = {
                    type = "boolean",
                    default = false,
                },
                creator_can_rename = {
                    type = "boolean",
                    default = false,
                },
                generate_transcript_on_close = {
                    type = "boolean",
                    default = true,
                },
                transcript_max_messages = {
                    type = "integer",
                    minimum = 1,
                    maximum = 1000,
                    default = 500,
                },
            },
        },
    },

    on_command = function(command, ctx)
        if command ~= "ticket" then return {} end

        local config = ctx.config or {}
        local roles = support_roles(config)
        local options = ctx.options or {}

        if options.open then
            local configuration_error = require_categories(config)
            if configuration_error then return reply(configuration_error) end

            local queue = normalize_identifier(
                options.open.queue,
                normalize_identifier(config.default_queue, "support")
            )
            if not value_in_array(queue, config.allowed_queues) then
                return reply("Ta kolejka nie jest dozwolona przez konfigurację serwera.")
            end
            return ticket({
                type = "open",
                subject = options.open.subject,
                description = options.open.description,
                queue = queue,
                policy = open_policy(config),
            })
        end

        if options.list then
            local scope = options.list.scope or "mine"
            if scope == "all" and not is_staff(ctx, roles) then
                return reply("Lista wszystkich ticketów jest dostępna tylko dla zespołu wsparcia.")
            end
            return ticket({
                type = "list",
                scope = scope,
                limit = scope == "all" and 25 or 15,
                support_role_ids = roles,
            })
        end

        if options.info then
            return ticket({ type = "info" })
        end

        if options.claim then
            if not is_staff(ctx, roles) then
                return reply("Przejęcie ticketu jest dostępne tylko dla zespołu wsparcia.")
            end
            return ticket({
                type = "claim",
                support_role_ids = roles,
            })
        end

        if options.unclaim then
            if not is_staff(ctx, roles) then
                return reply("Zwolnienie ticketu jest dostępne tylko dla zespołu wsparcia.")
            end
            return ticket({
                type = "unclaim",
                support_role_ids = roles,
            })
        end

        if options.close then
            return ticket({
                type = "close",
                reason = options.close.reason,
                policy = close_policy(config),
            })
        end

        if options.reopen then
            local configuration_error = require_categories(config)
            if configuration_error then return reply(configuration_error) end
            if not is_staff(ctx, roles) then
                return reply("Ponowne otwarcie ticketu jest dostępne tylko dla zespołu wsparcia.")
            end
            return ticket({
                type = "reopen",
                open_category_id = config.open_category_id,
                support_role_ids = roles,
            })
        end

        if options.add then
            return ticket({
                type = "add_member",
                user_id = options.add.user,
                support_role_ids = roles,
                creator_can_manage_participants = config.creator_can_manage_participants == true,
            })
        end

        if options.remove then
            return ticket({
                type = "remove_member",
                user_id = options.remove.user,
                support_role_ids = roles,
                creator_can_manage_participants = config.creator_can_manage_participants == true,
            })
        end

        if options.rename then
            return ticket({
                type = "rename",
                name = options.rename.name,
                support_role_ids = roles,
                creator_can_rename = config.creator_can_rename == true,
            })
        end

        if options.priority then
            if not is_staff(ctx, roles) then
                return reply("Priorytet ticketu może zmienić tylko zespół wsparcia.")
            end
            return ticket({
                type = "set_priority",
                priority = options.priority.level,
                support_role_ids = roles,
            })
        end

        if options.transcript then
            local maximum = bounded_integer(
                options.transcript.messages,
                bounded_integer(config.transcript_max_messages, 500, 1, 1000),
                1,
                1000
            )
            return ticket({
                type = "transcript",
                support_role_ids = roles,
                log_channel_id = optional_id(config.log_channel_id),
                max_messages = maximum,
            })
        end

        return reply("Wybierz jedną z podkomend `/ticket`.")
    end,
}

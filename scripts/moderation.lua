local MODERATE_MEMBERS = "1099511627776"

local function escalation_rules(config)
    local timeout_at = tonumber(config.timeout_at_points) or 3
    local kick_at = tonumber(config.kick_at_points) or 7
    local ban_at = tonumber(config.ban_at_points) or 10
    local timeout_seconds = tonumber(config.escalation_timeout_seconds) or 3600
    local delete_message_days = tonumber(config.ban_delete_message_days) or 1

    if timeout_at < 1 or timeout_at >= kick_at or kick_at >= ban_at or ban_at > 1000 then
        timeout_at = 3
        kick_at = 7
        ban_at = 10
    end
    if timeout_seconds < 1 or timeout_seconds > 2419200 then
        timeout_seconds = 3600
    end
    if delete_message_days < 0 or delete_message_days > 7 then
        delete_message_days = 1
    end

    return {
        {
            threshold_points = timeout_at,
            action = "timeout",
            duration_seconds = timeout_seconds,
        },
        { threshold_points = kick_at, action = "kick" },
        {
            threshold_points = ban_at,
            action = "ban",
            delete_message_days = delete_message_days,
        },
    }
end

local function missing(message)
    return {
        {
            type = "reply",
            content = message,
            ephemeral = true,
        },
    }
end

local function create_case(user_id, case_type, reason, metadata)
    return {
        type = "create_moderation_case",
        target_user_id = user_id,
        case_type = case_type,
        reason = reason,
        points = 0,
        metadata = metadata or { source = "manual_action" },
    }
end

return {
    manifest = {
        id = "moderation",
        name = "Moderation",
        version = "0.3.0",
        description = "Trwałe sprawy, ostrzeżenia, eskalacje i bezpieczne akcje moderacyjne.",
        category = "safety",
        default_enabled = true,
        commands = {
            {
                name = "moderate",
                description = "Wykonuje timeout, kick, ban albo czyszczenie wiadomości.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = MODERATE_MEMBERS,
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
            {
                name = "warn",
                description = "Dodaje trwałe ostrzeżenie i stosuje politykę eskalacji.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = MODERATE_MEMBERS,
                options = {
                    {
                        type = "user",
                        name = "user",
                        description = "Użytkownik otrzymujący ostrzeżenie.",
                        required = true,
                    },
                    {
                        type = "string",
                        name = "reason",
                        description = "Powód ostrzeżenia.",
                        required = true,
                        min_length = 1,
                        max_length = 512,
                    },
                    {
                        type = "integer",
                        name = "points",
                        description = "Liczba punktów ostrzeżenia.",
                        required = false,
                        min_value = 1,
                        max_value = 10,
                    },
                    {
                        type = "integer",
                        name = "expires_days",
                        description = "Po ilu dniach punkty wygasają; 0 oznacza bezterminowo.",
                        required = false,
                        min_value = 0,
                        max_value = 365,
                    },
                },
            },
            {
                name = "warnings",
                description = "Wyświetla trwałą historię moderacyjną użytkownika.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = MODERATE_MEMBERS,
                options = {
                    {
                        type = "user",
                        name = "user",
                        description = "Użytkownik, którego historię chcesz sprawdzić.",
                        required = true,
                    },
                    {
                        type = "boolean",
                        name = "include_resolved",
                        description = "Uwzględnia również zamknięte sprawy.",
                        required = false,
                    },
                    {
                        type = "integer",
                        name = "limit",
                        description = "Maksymalna liczba spraw.",
                        required = false,
                        min_value = 1,
                        max_value = 25,
                    },
                },
            },
            {
                name = "case-resolve",
                description = "Zamyka sprawę moderacyjną wraz z rozstrzygnięciem.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = MODERATE_MEMBERS,
                options = {
                    {
                        type = "integer",
                        name = "case_id",
                        description = "Numer sprawy.",
                        required = true,
                        min_value = 1,
                    },
                    {
                        type = "string",
                        name = "resolution",
                        description = "Powód zamknięcia sprawy.",
                        required = true,
                        min_length = 1,
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
                warning_expiry_days = {
                    type = "integer",
                    minimum = 0,
                    maximum = 365,
                    default = 90,
                },
                timeout_at_points = {
                    type = "integer",
                    minimum = 1,
                    maximum = 1000,
                    default = 3,
                },
                escalation_timeout_seconds = {
                    type = "integer",
                    minimum = 1,
                    maximum = 2419200,
                    default = 3600,
                },
                kick_at_points = {
                    type = "integer",
                    minimum = 2,
                    maximum = 1000,
                    default = 7,
                },
                ban_at_points = {
                    type = "integer",
                    minimum = 3,
                    maximum = 1000,
                    default = 10,
                },
                ban_delete_message_days = {
                    type = "integer",
                    minimum = 0,
                    maximum = 7,
                    default = 1,
                },
            },
        },
    },

    on_command = function(command, ctx)
        if command == "warn" then
            local days = tonumber(ctx.options.expires_days)
                or tonumber(ctx.config.warning_expiry_days)
                or 90
            if days < 0 or days > 365 then
                days = 90
            end
            local expires_in_seconds = nil
            if days > 0 then
                expires_in_seconds = days * 86400
            end

            return {
                {
                    type = "reply",
                    content = "Ostrzeżenie zostało przekazane do trwałego systemu spraw.",
                    ephemeral = true,
                },
                {
                    type = "create_moderation_case",
                    target_user_id = ctx.options.user,
                    case_type = "warning",
                    reason = ctx.options.reason,
                    points = tonumber(ctx.options.points) or 1,
                    expires_in_seconds = expires_in_seconds,
                    metadata = { source = "manual_warning" },
                    escalation_rules = escalation_rules(ctx.config),
                },
            }
        end

        if command == "warnings" then
            return {
                {
                    type = "reply",
                    content = "Pobieram historię moderacyjną.",
                    ephemeral = true,
                },
                {
                    type = "list_moderation_cases",
                    target_user_id = ctx.options.user,
                    include_resolved = ctx.options.include_resolved == true,
                    limit = tonumber(ctx.options.limit) or 10,
                },
            }
        end

        if command == "case-resolve" then
            return {
                {
                    type = "reply",
                    content = "Zamykam wskazaną sprawę.",
                    ephemeral = true,
                },
                {
                    type = "resolve_moderation_case",
                    case_id = tonumber(ctx.options.case_id),
                    resolution = ctx.options.resolution,
                },
            }
        end

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
            local seconds = tonumber(ctx.options.seconds) or default_seconds
            return {
                accepted,
                create_case(user_id, "timeout", reason, {
                    source = "manual_action",
                    duration_seconds = seconds,
                }),
                {
                    type = "timeout_member",
                    user_id = user_id,
                    seconds = seconds,
                    reason = reason,
                },
            }
        end

        if action == "kick" then
            return {
                accepted,
                create_case(user_id, "kick", reason),
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
                create_case(user_id, "ban", reason),
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

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
        version = "0.3.0",
        description = "Bezpieczne akcje moderacyjne, trwałe numerowane sprawy (cases), ostrzeżenia i audyt.",
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
                    {
                        type = "string",
                        name = "severity",
                        description = "Poziom powagi sprawy.",
                        required = false,
                        choices = {
                            { name = "Informacyjny", value = "info" },
                            { name = "Niski", value = "low" },
                            { name = "Średni", value = "medium" },
                            { name = "Wysoki", value = "high" },
                            { name = "Krytyczny", value = "critical" },
                        },
                    },
                    {
                        type = "integer",
                        name = "points",
                        description = "Punkty karne (0-1000).",
                        required = false,
                        min_value = 0,
                        max_value = 1000,
                    },
                },
            },
            {
                name = "warn",
                description = "Nakłada oficjalne ostrzeżenie i rejestruje sprawę moderacyjną.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "user",
                        name = "user",
                        description = "Ostrzegany użytkownik.",
                        required = true,
                    },
                    {
                        type = "string",
                        name = "reason",
                        description = "Powód ostrzeżenia.",
                        required = true,
                        max_length = 512,
                    },
                    {
                        type = "string",
                        name = "severity",
                        description = "Poziom powagi sprawy.",
                        required = false,
                        choices = {
                            { name = "Niski", value = "low" },
                            { name = "Średni", value = "medium" },
                            { name = "Wysoki", value = "high" },
                            { name = "Krytyczny", value = "critical" },
                        },
                    },
                    {
                        type = "integer",
                        name = "points",
                        description = "Punkty karne (domyślnie 1).",
                        required = false,
                        min_value = 0,
                        max_value = 1000,
                    },
                },
            },
            {
                name = "case",
                description = "Zarządzanie sprawami moderacyjnymi: podgląd, notatki, unieważnianie, statystyki.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "action",
                        description = "Operacja na sprawach moderacyjnych.",
                        required = true,
                        choices = {
                            { name = "Podgląd sprawy", value = "view" },
                            { name = "Lista spraw", value = "list" },
                            { name = "Dodaj notatkę", value = "note" },
                            { name = "Unieważnij sprawę", value = "void" },
                            { name = "Przywróć sprawę", value = "restore" },
                            { name = "Statystyki moderacji", value = "stats" },
                        },
                    },
                    {
                        type = "integer",
                        name = "case_number",
                        description = "Numer sprawy.",
                        required = false,
                        min_value = 1,
                    },
                    {
                        type = "user",
                        name = "user",
                        description = "Użytkownik (dla listy spraw lub statystyk).",
                        required = false,
                    },
                    {
                        type = "string",
                        name = "text",
                        description = "Treść notatki lub powód unieważnienia.",
                        required = false,
                        max_length = 1000,
                    },
                    {
                        type = "boolean",
                        name = "visible",
                        description = "Czy notatka ma być widoczna dla ukaranego użytkownika.",
                        required = false,
                    },
                    {
                        type = "integer",
                        name = "version",
                        description = "Wersja sprawy (dla void/restore).",
                        required = false,
                        min_value = 1,
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
        local accepted = {
            type = "reply",
            content = "Akcja moderacyjna została przyjęta.",
            ephemeral = true,
        }

        if command == "warn" then
            local user_id = ctx.options.user
            if not user_id then
                return missing("Musisz wskazać użytkownika do ostrzeżenia.")
            end
            local reason = ctx.options.reason or "Brak powodu"
            local severity = ctx.options.severity or "medium"
            local points = tonumber(ctx.options.points) or 1

            return {
                accepted,
                {
                    type = "moderation_case",
                    operation = {
                        type = "create",
                        subject_user_id = user_id,
                        kind = "warning",
                        reason = reason,
                        severity = severity,
                        points = points,
                        visible_to_subject = true,
                    },
                },
            }
        end

        if command == "case" then
            local action = ctx.options.action
            if action == "view" then
                local num = tonumber(ctx.options.case_number)
                if not num then
                    return missing("Musisz podać numer sprawy (case_number).")
                end
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "view",
                            case_number = num,
                        },
                    },
                }
            elseif action == "list" then
                local user_id = ctx.options.user
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "list",
                            subject_user_id = user_id,
                            limit = 15,
                            visible_to_subject_only = false,
                        },
                    },
                }
            elseif action == "note" then
                local num = tonumber(ctx.options.case_number)
                local text = ctx.options.text
                if not num or not text or text == "" then
                    return missing("Dodanie notatki wymaga numeru sprawy i tekstu (case_number, text).")
                end
                local visible = ctx.options.visible == true
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "add_note",
                            case_number = num,
                            body = text,
                            visible_to_subject = visible,
                        },
                    },
                }
            elseif action == "void" then
                local num = tonumber(ctx.options.case_number)
                local reason = ctx.options.text or "Unieważniono decyzją moderatora"
                local version = tonumber(ctx.options.version) or 0
                if not num then
                    return missing("Unieważnienie wymaga numeru sprawy (case_number).")
                end
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "void",
                            case_number = num,
                            reason = reason,
                            expected_version = version,
                        },
                    },
                }
            elseif action == "restore" then
                local num = tonumber(ctx.options.case_number)
                local version = tonumber(ctx.options.version) or 0
                if not num then
                    return missing("Przywrócenie wymaga numeru sprawy (case_number).")
                end
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "restore",
                            case_number = num,
                            expected_version = version,
                        },
                    },
                }
            elseif action == "stats" then
                local user_id = ctx.options.user
                return {
                    accepted,
                    {
                        type = "moderation_case",
                        operation = {
                            type = "stats",
                            moderator_user_id = user_id,
                        },
                    },
                }
            end

            return missing("Nieznana operacja na sprawie.")
        end

        if command ~= "moderate" then
            return {}
        end

        local action = ctx.options.action
        local user_id = ctx.options.user
        local reason = ctx.options.reason or "Brak powodu"
        local severity = ctx.options.severity or "medium"
        local points = tonumber(ctx.options.points) or 0

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
                {
                    type = "timeout_member",
                    user_id = user_id,
                    seconds = seconds,
                    reason = reason,
                },
                {
                    type = "moderation_case",
                    operation = {
                        type = "create",
                        subject_user_id = user_id,
                        kind = "timeout",
                        reason = reason,
                        severity = severity,
                        points = points > 0 and points or 2,
                        visible_to_subject = true,
                        duration_seconds = seconds,
                    },
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
                {
                    type = "moderation_case",
                    operation = {
                        type = "create",
                        subject_user_id = user_id,
                        kind = "kick",
                        reason = reason,
                        severity = severity,
                        points = points > 0 and points or 5,
                        visible_to_subject = true,
                    },
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
                {
                    type = "moderation_case",
                    operation = {
                        type = "create",
                        subject_user_id = user_id,
                        kind = "ban",
                        reason = reason,
                        severity = severity,
                        points = points > 0 and points or 10,
                        visible_to_subject = true,
                    },
                },
            }
        end

        return missing("Nieznana akcja moderacyjna.")
    end,
}

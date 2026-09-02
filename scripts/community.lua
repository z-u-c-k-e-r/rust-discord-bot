local function reply(content)
    return {
        {
            type = "reply",
            content = zuckerbot.truncate(content, 2000),
            ephemeral = true,
        },
    }
end

local function safe(value, limit)
    return zuckerbot.truncate(zuckerbot.escape_mentions(value or ""), limit or 1500)
end

local function destination(ctx, key, label)
    local value = ctx.config[key]
    if not value or value == "" then
        return nil, reply("Moduł nie ma skonfigurowanego kanału dla: " .. label .. ".")
    end
    return value, nil
end

local function accepted(message, channel_id, event, data)
    return {
        {
            type = "reply",
            content = message,
            ephemeral = true,
        },
        {
            type = "send_message",
            channel_id = channel_id,
            content = data.message,
        },
        {
            type = "audit",
            event = event,
            data = data.audit,
        },
    }
end

return {
    manifest = {
        id = "community",
        name = "Community Hub",
        version = "1.0.0",
        description = "Sugestie, prywatne zgłoszenia, feedback, podania oraz raporty błędów.",
        category = "community",
        default_enabled = false,
        commands = {
            {
                name = "suggest",
                description = "Przekazuje sugestię na skonfigurowany kanał społeczności.",
                dm_permission = false,
                options = {
                    {
                        type = "string",
                        name = "category",
                        description = "Kategoria sugestii.",
                        required = false,
                        choices = {
                            { name = "Serwer", value = "server" },
                            { name = "Discord", value = "discord" },
                            { name = "Event", value = "event" },
                            { name = "Inne", value = "other" },
                        },
                    },
                    { type = "string", name = "text", description = "Treść sugestii.", required = true, min_length = 5, max_length = 1500 },
                },
            },
            {
                name = "report",
                description = "Wysyła prywatne zgłoszenie do zespołu serwera.",
                dm_permission = false,
                options = {
                    { type = "user", name = "user", description = "Opcjonalnie zgłaszany użytkownik.", required = false },
                    { type = "string", name = "reason", description = "Powód zgłoszenia.", required = true, min_length = 5, max_length = 1200 },
                    { type = "string", name = "evidence", description = "Link, ID wiadomości albo dodatkowy kontekst.", required = false, max_length = 500 },
                },
            },
            {
                name = "feedback",
                description = "Przekazuje ocenę i opinię administratorom.",
                dm_permission = false,
                options = {
                    { type = "integer", name = "rating", description = "Ocena od 1 do 5.", required = true, min_value = 1, max_value = 5 },
                    { type = "string", name = "message", description = "Treść opinii.", required = true, min_length = 3, max_length = 1200 },
                },
            },
            {
                name = "apply",
                description = "Składa podanie do wybranego działu albo zespołu.",
                dm_permission = false,
                options = {
                    { type = "string", name = "topic", description = "Stanowisko, dział albo typ podania.", required = true, min_length = 2, max_length = 80 },
                    { type = "string", name = "application", description = "Treść podania.", required = true, min_length = 20, max_length = 1500 },
                },
            },
            {
                name = "bugreport",
                description = "Zgłasza błąd wraz z krokami jego odtworzenia.",
                dm_permission = false,
                options = {
                    {
                        type = "string",
                        name = "severity",
                        description = "Wpływ błędu.",
                        required = true,
                        choices = {
                            { name = "Niski", value = "low" },
                            { name = "Średni", value = "medium" },
                            { name = "Wysoki", value = "high" },
                            { name = "Krytyczny", value = "critical" },
                        },
                    },
                    { type = "string", name = "description", description = "Co nie działa?", required = true, min_length = 5, max_length = 900 },
                    { type = "string", name = "steps", description = "Jak odtworzyć problem?", required = false, max_length = 700 },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                suggestion_channel_id = { type = "string", title = "Kanał sugestii" },
                report_channel_id = { type = "string", title = "Prywatny kanał zgłoszeń" },
                feedback_channel_id = { type = "string", title = "Kanał feedbacku" },
                application_channel_id = { type = "string", title = "Kanał podań" },
                bug_channel_id = { type = "string", title = "Kanał raportów błędów" },
            },
        },
    },

    on_command = function(command, ctx)
        if command == "suggest" then
            local channel_id, error_reply = destination(ctx, "suggestion_channel_id", "sugestie")
            if not channel_id then return error_reply end
            local category = ctx.options.category or "other"
            local text = safe(ctx.options.text)
            return accepted(
                "Sugestia została przekazana. Dziękujemy!",
                channel_id,
                "suggestion_created",
                {
                    message = string.format("💡 **Nowa sugestia**\nAutor: <@%s> (`%s`)\nKategoria: `%s`\n\n%s", ctx.user_id, ctx.user_id, category, text),
                    audit = { author_id = ctx.user_id, category = category },
                }
            )
        end

        if command == "report" then
            local channel_id, error_reply = destination(ctx, "report_channel_id", "zgłoszenia")
            if not channel_id then return error_reply end
            local target = ctx.options.user
            local target_line = target and ("<@" .. target .. "> (`" .. target .. "`)") or "Nie wskazano"
            local evidence = safe(ctx.options.evidence or "Nie podano", 500)
            local reason = safe(ctx.options.reason, 1200)
            return accepted(
                "Zgłoszenie zostało bezpiecznie przekazane zespołowi.",
                channel_id,
                "user_report_created",
                {
                    message = string.format("🚨 **Prywatne zgłoszenie**\nZgłaszający: <@%s> (`%s`)\nCel: %s\nDowody/kontekst: %s\n\n**Powód:**\n%s", ctx.user_id, ctx.user_id, target_line, evidence, reason),
                    audit = { reporter_id = ctx.user_id, target_user_id = target, has_evidence = ctx.options.evidence ~= nil },
                }
            )
        end

        if command == "feedback" then
            local channel_id, error_reply = destination(ctx, "feedback_channel_id", "feedback")
            if not channel_id then return error_reply end
            local rating = math.tointeger(ctx.options.rating) or 1
            local stars = string.rep("⭐", rating)
            local message = safe(ctx.options.message, 1200)
            return accepted(
                "Opinia została przekazana. Dziękujemy!",
                channel_id,
                "feedback_created",
                {
                    message = string.format("📝 **Nowy feedback**\nAutor: <@%s> (`%s`)\nOcena: %s (`%d/5`)\n\n%s", ctx.user_id, ctx.user_id, stars, rating, message),
                    audit = { author_id = ctx.user_id, rating = rating },
                }
            )
        end

        if command == "apply" then
            local channel_id, error_reply = destination(ctx, "application_channel_id", "podania")
            if not channel_id then return error_reply end
            local topic = safe(ctx.options.topic, 80)
            local application = safe(ctx.options.application, 1500)
            return accepted(
                "Podanie zostało przekazane do rozpatrzenia.",
                channel_id,
                "application_created",
                {
                    message = string.format("📨 **Nowe podanie**\nKandydat: <@%s> (`%s`)\nTemat: **%s**\n\n%s", ctx.user_id, ctx.user_id, topic, application),
                    audit = { applicant_id = ctx.user_id, topic = topic },
                }
            )
        end

        if command == "bugreport" then
            local channel_id, error_reply = destination(ctx, "bug_channel_id", "raporty błędów")
            if not channel_id then return error_reply end
            local severity = ctx.options.severity
            local description = safe(ctx.options.description, 900)
            local steps = safe(ctx.options.steps or "Nie podano", 700)
            return accepted(
                "Raport błędu został przekazany.",
                channel_id,
                "bug_report_created",
                {
                    message = string.format("🐞 **Raport błędu**\nAutor: <@%s> (`%s`)\nWażność: `%s`\n\n**Opis:**\n%s\n\n**Kroki odtworzenia:**\n%s", ctx.user_id, ctx.user_id, severity, description, steps),
                    audit = { author_id = ctx.user_id, severity = severity },
                }
            )
        end

        return {}
    end,
}

local function reply(content, ephemeral)
    return {
        {
            type = "reply",
            content = zuckerbot.truncate(content, 2000),
            ephemeral = ephemeral,
        },
    }
end

local function show(ctx, key, heading)
    local value = ctx.config[key]
    if not value or value == "" then
        return reply("Administratorzy nie uzupełnili jeszcze sekcji: **" .. heading .. "**.", true)
    end
    local prefix = ctx.config.show_headings == false and "" or (heading .. "\n\n")
    return reply(prefix .. value, ctx.config.ephemeral_responses == true)
end

return {
    manifest = {
        id = "server_info",
        name = "Server Information Center",
        version = "1.0.0",
        description = "Konfigurowalne komendy z regulaminem, FAQ, linkami, personelem i harmonogramem.",
        category = "information",
        default_enabled = false,
        commands = {
            { name = "rules", description = "Wyświetla regulamin serwera.", dm_permission = false },
            { name = "faq", description = "Wyświetla najczęstsze pytania i odpowiedzi.", dm_permission = false },
            { name = "links", description = "Wyświetla oficjalne linki społeczności.", dm_permission = false },
            { name = "supportinfo", description = "Wyświetla instrukcję uzyskania pomocy.", dm_permission = false },
            { name = "staff", description = "Wyświetla informacje o zespole serwera.", dm_permission = false },
            { name = "schedule", description = "Wyświetla harmonogram wydarzeń albo aktywności.", dm_permission = false },
            { name = "aboutserver", description = "Wyświetla opis i najważniejsze informacje o serwerze.", dm_permission = false },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                rules = { type = "string", title = "Regulamin", maxLength = 1900 },
                faq = { type = "string", title = "FAQ", maxLength = 1900 },
                links = { type = "string", title = "Oficjalne linki", maxLength = 1900 },
                support = { type = "string", title = "Informacje o pomocy", maxLength = 1900 },
                staff = { type = "string", title = "Zespół", maxLength = 1900 },
                schedule = { type = "string", title = "Harmonogram", maxLength = 1900 },
                about = { type = "string", title = "O serwerze", maxLength = 1900 },
                show_headings = { type = "boolean", default = true },
                ephemeral_responses = { type = "boolean", default = false },
            },
        },
    },

    on_command = function(command, ctx)
        if command == "rules" then return show(ctx, "rules", "📜 **Regulamin serwera**") end
        if command == "faq" then return show(ctx, "faq", "❓ **Najczęstsze pytania**") end
        if command == "links" then return show(ctx, "links", "🔗 **Oficjalne linki**") end
        if command == "supportinfo" then return show(ctx, "support", "🛟 **Pomoc i wsparcie**") end
        if command == "staff" then return show(ctx, "staff", "🛡️ **Zespół serwera**") end
        if command == "schedule" then return show(ctx, "schedule", "📅 **Harmonogram**") end
        if command == "aboutserver" then return show(ctx, "about", "ℹ️ **O serwerze**") end
        return {}
    end,
}

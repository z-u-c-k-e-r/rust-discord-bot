local memes = {
    "Programista: naprawiłem błąd. Tester: który? Programista: ten, który widziałem.",
    "Kod działa. Nikt nie wie dlaczego. Nie dotykać przed wdrożeniem.",
    "To nie bug. To nieudokumentowana mechanika serwera.",
    "Najkrótszy horror programisty: działało na moim komputerze.",
    "Administrator nie śpi. Administrator czeka na kolejny alert.",
}

local answers = {
    "Zdecydowanie tak.",
    "Raczej tak.",
    "Wygląda obiecująco.",
    "Zapytaj ponownie później.",
    "Nie liczyłbym na to.",
    "Zdecydowanie nie.",
}

return {
    manifest = {
        id = "fun",
        name = "Fun and Memes",
        version = "0.2.0",
        description = "Memy tekstowe, losowanie, wybór i odpowiedzi społecznościowe.",
        category = "engagement",
        default_enabled = true,
        commands = {
            {
                name = "meme",
                description = "Losuje mem tekstowy.",
                integration_types = { "guild", "user" },
                contexts = { "guild", "bot_dm", "private_channel" },
            },
            {
                name = "roll",
                description = "Losuje liczbę od 1 do podanego maksimum.",
                integration_types = { "guild", "user" },
                contexts = { "guild", "bot_dm", "private_channel" },
                options = {
                    {
                        type = "integer",
                        name = "max",
                        description = "Najwyższa możliwa liczba.",
                        required = false,
                        min_value = 2,
                        max_value = 1000000,
                    },
                },
            },
            {
                name = "choose",
                description = "Wybiera jedną pozycję z listy rozdzielonej przecinkami.",
                integration_types = { "guild", "user" },
                contexts = { "guild", "bot_dm", "private_channel" },
                options = {
                    {
                        type = "string",
                        name = "options",
                        description = "Na przykład: tank, healer, damage.",
                        required = true,
                        min_length = 3,
                        max_length = 500,
                    },
                },
            },
            {
                name = "eightball",
                description = "Odpowiada na pytanie jak magiczna kula.",
                integration_types = { "guild", "user" },
                contexts = { "guild", "bot_dm", "private_channel" },
                options = {
                    {
                        type = "string",
                        name = "question",
                        description = "Pytanie, na które ma odpowiedzieć bot.",
                        required = true,
                        min_length = 2,
                        max_length = 300,
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
        local user_suffix = tonumber(string.sub(ctx.user_id or "", -8)) or 1
        math.randomseed(zuckerbot.unix_time() + user_suffix)

        if command == "meme" then
            return {
                {
                    type = "reply",
                    content = memes[math.random(#memes)],
                    ephemeral = false,
                },
            }
        end

        if command == "roll" then
            local raw_max = math.tointeger(ctx.options.max) or 100
            local maximum = math.max(2, raw_max)
            return {
                {
                    type = "reply",
                    content = string.format("%s wylosował(a): **%d** (1–%d)", zuckerbot.escape_mentions(ctx.user_name), math.random(maximum), maximum),
                    ephemeral = false,
                },
            }
        end

        if command == "choose" then
            local values = {}
            for value in string.gmatch(ctx.options.options or "", "([^,]+)") do
                value = string.gsub(value, "^%s*(.-)%s*$", "%1")
                if value ~= "" then
                    table.insert(values, value)
                end
            end

            if #values < 2 then
                return {
                    {
                        type = "reply",
                        content = "Podaj co najmniej dwie opcje rozdzielone przecinkami.",
                        ephemeral = true,
                    },
                }
            end

            return {
                {
                    type = "reply",
                    content = "Wybieram: **" .. zuckerbot.escape_mentions(values[math.random(#values)]) .. "**",
                    ephemeral = false,
                },
            }
        end

        if command == "eightball" then
            local question = ctx.options.question or ""
            local answer = answers[math.random(#answers)]
            return {
                {
                    type = "reply",
                    content = string.format("> *%s*\n🎱 %s", zuckerbot.escape_mentions(question), answer),
                    ephemeral = false,
                },
            }
        end

        return {}
    end,
}

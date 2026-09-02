local function reply(content, ephemeral)
    return {
        {
            type = "reply",
            content = zuckerbot.truncate(content, 2000),
            ephemeral = ephemeral == true,
        },
    }
end

local function hash(value)
    local total = 2166136261
    for index = 1, #value do
        total = ((total ~ string.byte(value, index)) * 16777619) & 0x7FFFFFFF
    end
    return total
end

local function seed(ctx, extra)
    local source = (ctx.user_id or "") .. ":" .. tostring(zuckerbot.unix_time()) .. ":" .. (extra or "")
    math.randomseed(hash(source))
end

local function trim(value)
    return string.match(value, "^%s*(.-)%s*$") or ""
end

local function split_people(value)
    local people = {}
    local seen = {}
    for item in string.gmatch((value or "") .. ",", "(.-),") do
        item = trim(item)
        local key = string.lower(item)
        if item ~= "" and not seen[key] then
            seen[key] = true
            table.insert(people, item)
        end
    end
    return people
end

local function shuffle(values)
    for index = #values, 2, -1 do
        local other = math.random(index)
        values[index], values[other] = values[other], values[index]
    end
end

return {
    manifest = {
        id = "games",
        name = "Community Games",
        version = "1.0.0",
        description = "Lekkie gry społecznościowe, losowania i generatory bez waluty ani prawdziwych stawek.",
        category = "fun",
        default_enabled = true,
        commands = {
            {
                name = "coinflip",
                description = "Rzuca wirtualną monetą.",
                dm_permission = true,
            },
            {
                name = "dice",
                description = "Rzuca wybraną liczbą kostek.",
                dm_permission = true,
                options = {
                    { type = "integer", name = "count", description = "Liczba kostek.", required = false, min_value = 1, max_value = 20 },
                    { type = "integer", name = "sides", description = "Liczba ścian każdej kostki.", required = false, min_value = 2, max_value = 100 },
                },
            },
            {
                name = "rps",
                description = "Gra w papier, kamień, nożyce przeciwko botowi.",
                dm_permission = true,
                options = {
                    {
                        type = "string",
                        name = "choice",
                        description = "Twój wybór.",
                        required = true,
                        choices = {
                            { name = "Kamień", value = "rock" },
                            { name = "Papier", value = "paper" },
                            { name = "Nożyce", value = "scissors" },
                        },
                    },
                },
            },
            {
                name = "rate",
                description = "Wystawia stabilną ocenę od 0 do 100 procent.",
                dm_permission = true,
                options = {
                    { type = "string", name = "subject", description = "Co mamy ocenić?", required = true, min_length = 1, max_length = 200 },
                },
            },
            {
                name = "ship",
                description = "Oblicza zabawne dopasowanie dwóch nazw.",
                dm_permission = true,
                options = {
                    { type = "string", name = "first", description = "Pierwsza nazwa.", required = true, min_length = 1, max_length = 64 },
                    { type = "string", name = "second", description = "Druga nazwa.", required = true, min_length = 1, max_length = 64 },
                },
            },
            {
                name = "teams",
                description = "Dzieli listę osób na losowe drużyny.",
                dm_permission = true,
                options = {
                    { type = "string", name = "players", description = "Nazwy rozdzielone przecinkami.", required = true, min_length = 3, max_length = 1000 },
                    { type = "integer", name = "count", description = "Liczba drużyn.", required = false, min_value = 2, max_value = 10 },
                },
            },
            {
                name = "duel",
                description = "Rozstrzyga towarzyski pojedynek dwóch użytkowników.",
                dm_permission = false,
                options = {
                    { type = "user", name = "opponent", description = "Twój przeciwnik.", required = true },
                },
            },
            {
                name = "lootdrop",
                description = "Generuje losowy fantastyczny łup bez wartości ekonomicznej.",
                dm_permission = true,
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                public_results = { type = "boolean", default = true },
            },
        },
    },

    on_command = function(command, ctx)
        local ephemeral = ctx.config.public_results == false

        if command == "coinflip" then
            seed(ctx, command)
            local result = math.random(2) == 1 and "🪙 Orzeł" or "🪙 Reszka"
            return reply(result, ephemeral)
        end

        if command == "dice" then
            local count = math.tointeger(ctx.options.count or 1) or 1
            local sides = math.tointeger(ctx.options.sides or 6) or 6
            seed(ctx, tostring(count) .. ":" .. tostring(sides))
            local values = {}
            local total = 0
            for _ = 1, count do
                local value = math.random(1, sides)
                total = total + value
                table.insert(values, tostring(value))
            end
            local content = "🎲 Wyniki: `" .. table.concat(values, ", ") .. "`"
            if count > 1 then content = content .. "\nSuma: **" .. total .. "**" end
            return reply(content, ephemeral)
        end

        if command == "rps" then
            local labels = { rock = "🪨 Kamień", paper = "📄 Papier", scissors = "✂️ Nożyce" }
            local choices = { "rock", "paper", "scissors" }
            seed(ctx, ctx.options.choice)
            local bot = choices[math.random(#choices)]
            local user = ctx.options.choice
            local outcome
            if user == bot then
                outcome = "Remis."
            elseif (user == "rock" and bot == "scissors")
                or (user == "paper" and bot == "rock")
                or (user == "scissors" and bot == "paper") then
                outcome = "Wygrywasz!"
            else
                outcome = "Bot wygrywa."
            end
            return reply("Ty: **" .. labels[user] .. "**\nBot: **" .. labels[bot] .. "**\n" .. outcome, ephemeral)
        end

        if command == "rate" then
            local subject = zuckerbot.escape_mentions(ctx.options.subject or "")
            local score = hash(string.lower(subject) .. ":" .. ctx.user_id) % 101
            return reply("Ocena dla **" .. subject .. "**: **" .. score .. "%**", ephemeral)
        end

        if command == "ship" then
            local first = zuckerbot.escape_mentions(trim(ctx.options.first or ""))
            local second = zuckerbot.escape_mentions(trim(ctx.options.second or ""))
            local pair = string.lower(first) < string.lower(second) and first .. ":" .. second or second .. ":" .. first
            local score = hash(string.lower(pair)) % 101
            local filled = math.floor(score / 10)
            local bar = string.rep("█", filled) .. string.rep("░", 10 - filled)
            return reply(string.format("💞 **%s + %s**\n`%s` **%d%%**", first, second, bar, score), ephemeral)
        end

        if command == "teams" then
            local people = split_people(ctx.options.players)
            local team_count = math.tointeger(ctx.options.count or 2) or 2
            if #people < team_count then
                return reply("Liczba różnych osób musi być co najmniej równa liczbie drużyn.", true)
            end
            if #people > 40 then
                return reply("Jednorazowo można podzielić maksymalnie 40 osób.", true)
            end

            seed(ctx, table.concat(people, ":"))
            shuffle(people)
            local teams = {}
            for index = 1, team_count do teams[index] = {} end
            for index, person in ipairs(people) do
                table.insert(teams[((index - 1) % team_count) + 1], zuckerbot.escape_mentions(person))
            end

            local lines = {}
            for index, team in ipairs(teams) do
                table.insert(lines, "**Drużyna " .. index .. ":** " .. table.concat(team, ", "))
            end
            return reply(table.concat(lines, "\n"), ephemeral)
        end

        if command == "duel" then
            local opponent = ctx.options.opponent
            if opponent == ctx.user_id then return reply("Nie możesz pojedynkować się sam ze sobą.", true) end
            seed(ctx, opponent)
            local first_wins = math.random(2) == 1
            local winner = first_wins and ctx.user_id or opponent
            local loser = first_wins and opponent or ctx.user_id
            local moves = { "potężnym ciosem", "perfekcyjnym unikiem", "sprytną kontrą", "krytycznym trafieniem" }
            return reply(string.format("⚔️ <@%s> pokonuje <@%s> %s!", winner, loser, moves[math.random(#moves)]), ephemeral)
        end

        if command == "lootdrop" then
            seed(ctx, command)
            local roll = math.random(1000)
            local rarity
            if roll == 1000 then rarity = "🌈 Mityczny"
            elseif roll >= 990 then rarity = "🟧 Legendarny"
            elseif roll >= 930 then rarity = "🟪 Epicki"
            elseif roll >= 750 then rarity = "🟦 Rzadki"
            elseif roll >= 400 then rarity = "🟩 Niezwykły"
            else rarity = "⬜ Zwykły" end
            local prefixes = { "Pradawny", "Runiczny", "Płonący", "Lodowy", "Astralny", "Przeklęty", "Królewski" }
            local items = { "Miecz", "Łuk", "Pierścień", "Amulet", "Hełm", "Tarcza", "Kostur", "Sztylet" }
            return reply("🎁 Zdobywasz: **" .. rarity .. " " .. prefixes[math.random(#prefixes)] .. " " .. items[math.random(#items)] .. "**", ephemeral)
        end

        return {}
    end,
}

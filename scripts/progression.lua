local ADMINISTRATOR = 8
local MANAGE_GUILD = 32

local function has_manage_guild(ctx)
    local permissions = math.tointeger(tonumber(ctx.member_permissions or "0")) or 0
    return (permissions & ADMINISTRATOR) == ADMINISTRATOR
        or (permissions & MANAGE_GUILD) == MANAGE_GUILD
end

local function contains(values, expected)
    expected = tostring(expected or "")
    for _, value in ipairs(values or {}) do
        if tostring(value) == expected then return true end
    end
    return false
end

local function starts_with_any(content, prefixes)
    for _, prefix in ipairs(prefixes or {}) do
        if prefix ~= "" and string.sub(content, 1, #prefix) == prefix then
            return true
        end
    end
    return false
end

local function hash(value)
    local result = 2166136261
    for index = 1, #value do
        result = ((result ~ string.byte(value, index)) * 16777619) & 0x7FFFFFFF
    end
    return result
end

local function action(operation)
    return {
        {
            type = "progression",
            operation = operation,
        },
    }
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

return {
    manifest = {
        id = "progression",
        name = "Progression and Economy",
        version = "1.0.0",
        description = "Trwały leveling, XP, monety, daily streak, transfery, reputacja i rankingi.",
        category = "engagement",
        default_enabled = false,
        events = {
            "message_create",
        },
        commands = {
            {
                name = "rank",
                description = "Wyświetla poziom, XP i pełny profil aktywności.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    { type = "user", name = "user", description = "Opcjonalny użytkownik.", required = false },
                },
            },
            {
                name = "balance",
                description = "Wyświetla saldo monet i serię nagród daily.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    { type = "user", name = "user", description = "Opcjonalny użytkownik.", required = false },
                },
            },
            {
                name = "leaderboard",
                description = "Wyświetla ranking aktywności, monet albo reputacji.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    {
                        type = "string",
                        name = "metric",
                        description = "Kryterium rankingu.",
                        required = true,
                        choices = {
                            { name = "XP", value = "xp" },
                            { name = "Monety", value = "coins" },
                            { name = "Reputacja", value = "reputation" },
                            { name = "Wiadomości", value = "messages" },
                        },
                    },
                    { type = "integer", name = "limit", description = "Liczba pozycji od 1 do 25.", required = false, min_value = 1, max_value = 25 },
                },
            },
            {
                name = "daily",
                description = "Odbiera codzienną nagrodę i zwiększa serię.",
                integration_types = { "guild" },
                contexts = { "guild" },
            },
            {
                name = "pay",
                description = "Przekazuje własne monety innemu użytkownikowi.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    { type = "user", name = "user", description = "Odbiorca monet.", required = true },
                    { type = "integer", name = "amount", description = "Kwota transferu.", required = true, min_value = 1, max_value = 1000000000 },
                },
            },
            {
                name = "rep",
                description = "Przyznaje reputację pomocnemu członkowi społeczności.",
                integration_types = { "guild" },
                contexts = { "guild" },
                options = {
                    { type = "user", name = "user", description = "Użytkownik otrzymujący reputację.", required = true },
                },
            },
            {
                name = "progressadmin",
                description = "Koryguje XP, monety albo reputację użytkownika.",
                integration_types = { "guild" },
                contexts = { "guild" },
                default_member_permissions = "32",
                options = {
                    { type = "user", name = "user", description = "Użytkownik do aktualizacji.", required = true },
                    { type = "integer", name = "xp", description = "Zmiana XP; dopuszczalna jest liczba ujemna.", required = false, max_value = 1000000000 },
                    { type = "integer", name = "coins", description = "Zmiana monet; dopuszczalna jest liczba ujemna.", required = false, max_value = 1000000000 },
                    { type = "integer", name = "reputation", description = "Zmiana reputacji; może być ujemna.", required = false, max_value = 1000000000 },
                    { type = "string", name = "reason", description = "Powód korekty do audytu.", required = false, max_length = 512 },
                },
            },
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                xp_min = { type = "integer", minimum = 1, maximum = 1000, default = 15 },
                xp_max = { type = "integer", minimum = 1, maximum = 1000, default = 25 },
                xp_cooldown_seconds = { type = "integer", minimum = 1, maximum = 86400, default = 60 },
                minimum_message_length = { type = "integer", minimum = 0, maximum = 2000, default = 3 },
                excluded_channel_ids = {
                    type = "array",
                    items = { type = "string" },
                    maxItems = 100,
                },
                ignored_prefixes = {
                    type = "array",
                    items = { type = "string", maxLength = 16 },
                    default = { "!", "/" },
                    maxItems = 20,
                },
                announce_level_up = { type = "boolean", default = true },
                level_up_message = {
                    type = "string",
                    maxLength = 500,
                    default = "🎉 {user} awansuje na poziom **{level}**! Łączne XP: **{xp}**.",
                },
                daily_base_reward = { type = "integer", minimum = 1, maximum = 1000000, default = 100 },
                daily_streak_bonus = { type = "integer", minimum = 0, maximum = 100000, default = 10 },
                daily_max_streak_bonus = { type = "integer", minimum = 0, maximum = 365, default = 30 },
                reputation_amount = { type = "integer", minimum = 1, maximum = 100, default = 1 },
                reputation_cooldown_seconds = { type = "integer", minimum = 60, maximum = 604800, default = 86400 },
            },
        },
    },

    on_command = function(command, ctx)
        if command == "rank" then
            return action({
                type = "profile",
                user_id = ctx.options.user,
            })
        end

        if command == "balance" then
            return action({
                type = "balance",
                user_id = ctx.options.user,
            })
        end

        if command == "leaderboard" then
            return action({
                type = "leaderboard",
                metric = ctx.options.metric,
                limit = math.tointeger(ctx.options.limit or 10) or 10,
            })
        end

        if command == "daily" then
            return action({
                type = "daily",
                base_reward = math.tointeger(ctx.config.daily_base_reward or 100) or 100,
                streak_bonus = math.tointeger(ctx.config.daily_streak_bonus or 10) or 10,
                max_streak_bonus = math.tointeger(ctx.config.daily_max_streak_bonus or 30) or 30,
            })
        end

        if command == "pay" then
            return action({
                type = "transfer",
                user_id = ctx.options.user,
                amount = math.tointeger(ctx.options.amount),
            })
        end

        if command == "rep" then
            return action({
                type = "give_reputation",
                user_id = ctx.options.user,
                amount = math.tointeger(ctx.config.reputation_amount or 1) or 1,
                cooldown_seconds = math.tointeger(ctx.config.reputation_cooldown_seconds or 86400) or 86400,
            })
        end

        if command == "progressadmin" then
            if not has_manage_guild(ctx) then
                return reply("Nie masz uprawnienia Zarządzanie serwerem.")
            end

            local xp = math.tointeger(ctx.options.xp or 0) or 0
            local coins = math.tointeger(ctx.options.coins or 0) or 0
            local reputation = math.tointeger(ctx.options.reputation or 0) or 0
            if xp == 0 and coins == 0 and reputation == 0 then
                return reply("Podaj co najmniej jedną niezerową zmianę.")
            end
            if math.abs(xp) > 1000000000
                or math.abs(coins) > 1000000000
                or math.abs(reputation) > 1000000000 then
                return reply("Jedna korekta nie może przekraczać miliarda w żadnym kierunku.")
            end

            return action({
                type = "adjust",
                user_id = ctx.options.user,
                xp_delta = xp,
                coins_delta = coins,
                reputation_delta = reputation,
                reason = ctx.options.reason,
            })
        end

        return {}
    end,

    on_event = function(event, ctx)
        if event ~= "message_create" then return {} end
        if not ctx.actor_id or not ctx.channel_id then return {} end
        if contains(ctx.config.excluded_channel_ids, ctx.channel_id) then return {} end

        local content = ctx.data.content or ""
        local minimum_length = math.tointeger(ctx.config.minimum_message_length or 3) or 3
        if #content < minimum_length then return {} end
        if starts_with_any(content, ctx.config.ignored_prefixes or { "!", "/" }) then return {} end

        local minimum = math.tointeger(ctx.config.xp_min or 15) or 15
        local maximum = math.tointeger(ctx.config.xp_max or 25) or 25
        minimum = math.max(1, math.min(1000, minimum))
        maximum = math.max(1, math.min(1000, maximum))
        if minimum > maximum then minimum, maximum = maximum, minimum end
        local range = maximum - minimum + 1
        local amount = minimum + (hash(tostring(ctx.data.message_id or content)) % range)

        return action({
            type = "award_message_xp",
            amount = amount,
            cooldown_seconds = math.tointeger(ctx.config.xp_cooldown_seconds or 60) or 60,
            announce_level_up = ctx.config.announce_level_up ~= false,
            level_up_message = ctx.config.level_up_message,
        })
    end,
}

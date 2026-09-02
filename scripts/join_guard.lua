local DISCORD_EPOCH_MS = 1420070400000

local function lower(value)
    return string.lower(value or "")
end

local function account_created_at(user_id)
    if not string.match(user_id or "", "^%d+$") then return nil end
    local value = math.tointeger(tonumber(user_id))
    if not value then return nil end
    return (((value >> 22) + DISCORD_EPOCH_MS) // 1000)
end

local function suspicious_name(name, fragments)
    local normalized = lower(name)
    for _, fragment in ipairs(fragments or {}) do
        fragment = lower(fragment)
        if fragment ~= "" and string.find(normalized, fragment, 1, true) then
            return fragment
        end
    end
    return nil
end

return {
    manifest = {
        id = "join_guard",
        name = "Join Guard",
        version = "1.0.0",
        description = "Loguje wejścia oraz alarmuje o bardzo nowych kontach, botach i podejrzanych nazwach.",
        category = "safety",
        default_enabled = false,
        events = {
            "guild_member_add",
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            required = { "alert_channel_id" },
            properties = {
                alert_channel_id = { type = "string", title = "Kanał alertów bezpieczeństwa" },
                join_log_channel_id = { type = "string", title = "Opcjonalny kanał logów wejść" },
                minimum_account_age_days = { type = "integer", minimum = 0, maximum = 3650, default = 7 },
                alert_for_bots = { type = "boolean", default = true },
                blocked_name_fragments = {
                    type = "array",
                    title = "Podejrzane fragmenty nazwy",
                    items = { type = "string", minLength = 1, maxLength = 32 },
                    maxItems = 100,
                },
                log_safe_joins = { type = "boolean", default = false },
            },
        },
    },

    on_event = function(event, ctx)
        if event ~= "guild_member_add" then return {} end
        local user = ctx.data.user or {}
        local created_at = account_created_at(tostring(user.id or ""))
        if not created_at then return {} end

        local now = zuckerbot.unix_time()
        local age_seconds = math.max(0, now - created_at)
        local age_days = math.floor(age_seconds / 86400)
        local minimum_age = tonumber(ctx.config.minimum_account_age_days) or 7
        local name = user.global_name or user.name or "Nieznany"
        local fragment = suspicious_name(name, ctx.config.blocked_name_fragments)

        local reasons = {}
        if age_days < minimum_age then
            table.insert(reasons, "konto ma tylko " .. age_days .. " dni")
        end
        if user.bot == true and ctx.config.alert_for_bots ~= false then
            table.insert(reasons, "dołączyło konto bota")
        end
        if fragment then
            table.insert(reasons, "nazwa zawiera zablokowany fragment: " .. fragment)
        end

        local actions = {}
        local identity = string.format("<@%s> (`%s`, %s)", tostring(user.id), tostring(user.id), zuckerbot.escape_mentions(name))

        if #reasons > 0 and ctx.config.alert_channel_id and ctx.config.alert_channel_id ~= "" then
            table.insert(actions, {
                type = "send_message",
                channel_id = ctx.config.alert_channel_id,
                content = string.format(
                    "🛡️ **Join Guard: konto wymaga uwagi**\nUżytkownik: %s\nUtworzenie konta: <t:%d:F> (<t:%d:R>)\nPowody: %s",
                    identity,
                    created_at,
                    created_at,
                    zuckerbot.escape_mentions(table.concat(reasons, "; "))
                ),
            })
        end

        local log_channel = ctx.config.join_log_channel_id
        if log_channel and log_channel ~= "" and (#reasons > 0 or ctx.config.log_safe_joins == true) then
            table.insert(actions, {
                type = "send_message",
                channel_id = log_channel,
                content = string.format(
                    "➡️ **Nowy członek**: %s\nWiek konta: **%d dni**\nOcena: **%s**",
                    identity,
                    age_days,
                    #reasons > 0 and "wymaga uwagi" or "bez alertów"
                ),
            })
        end

        table.insert(actions, {
            type = "audit",
            event = #reasons > 0 and "join_guard_flagged" or "join_guard_checked",
            data = {
                user_id = tostring(user.id),
                account_created_at = created_at,
                account_age_days = age_days,
                reasons = reasons,
            },
        })

        return actions
    end,
}

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
        name = "Join Guard & Anti-Raid",
        version = "1.1.0",
        description = "Wykrywa nowe konta, brak awatara, podejrzane nazwy oraz bursty dołączeń w trybie Anti-Raid.",
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
                minimum_account_age_hours = { type = "integer", minimum = 0, maximum = 87600, default = 24 },
                alert_for_no_avatar = { type = "boolean", default = true },
                alert_for_bots = { type = "boolean", default = true },
                blocked_name_fragments = {
                    type = "array",
                    title = "Podejrzane fragmenty nazwy",
                    items = { type = "string", minLength = 1, maxLength = 32 },
                    maxItems = 100,
                },
                raid_burst_threshold = { type = "integer", minimum = 2, maximum = 50, default = 5 },
                action_on_raid = {
                    type = "string",
                    enum = { "alert", "quarantine", "kick", "timeout" },
                    default = "alert",
                },
                quarantine_role_id = { type = "string", title = "ID roli kwarantanny dla nowych kont w czasie rajdu" },
                raid_timeout_seconds = { type = "integer", minimum = 60, maximum = 2419200, default = 3600 },
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
        local age_hours = math.floor(age_seconds / 3600)
        local age_days = math.floor(age_seconds / 86400)
        local age_minutes = math.floor(age_seconds / 60)

        local minimum_hours = tonumber(ctx.config.minimum_account_age_hours)
        if minimum_hours == nil then
            minimum_hours = (tonumber(ctx.config.minimum_account_age_days) or 1) * 24
        end

        local name = user.global_name or user.name or "Nieznany"
        local fragment = suspicious_name(name, ctx.config.blocked_name_fragments)
        local has_avatar = user.has_avatar
        if has_avatar == nil and user.avatar ~= nil then
            has_avatar = true
        end

        local recent_joins = tonumber(ctx.data.recent_joins_count) or 1
        local burst_threshold = tonumber(ctx.config.raid_burst_threshold) or 5
        local is_raid = recent_joins >= burst_threshold

        local reasons = {}
        if age_hours < minimum_hours then
            if age_hours == 0 then
                table.insert(reasons, string.format("nowe konto (utworzone %d min temu)", age_minutes))
            else
                table.insert(reasons, string.format("młode konto (%d godz. / %d dni)", age_hours, age_days))
            end
        end

        if has_avatar == false and ctx.config.alert_for_no_avatar ~= false then
            table.insert(reasons, "brak awatara (domyślny awatar Discorda)")
        end

        if user.bot == true and ctx.config.alert_for_bots ~= false then
            table.insert(reasons, "dołączyło konto bota")
        end

        if fragment then
            table.insert(reasons, "nazwa zawiera zablokowany fragment: " .. fragment)
        end

        if is_raid then
            table.insert(reasons, string.format("🚨 WYKRYTO BURST / RAJD (%d wejść w oknie 60 s!)", recent_joins))
        end

        local actions = {}
        local identity = string.format("<@%s> (`%s`, %s)", tostring(user.id), tostring(user.id), zuckerbot.escape_mentions(name))

        if is_raid then
            local raid_action = ctx.config.action_on_raid or "alert"
            if raid_action == "kick" then
                table.insert(actions, {
                    type = "kick_member",
                    user_id = tostring(user.id),
                    reason = "JoinGuard: automatyczna ochrona przed rajdem (Lockdown)",
                })
            elseif raid_action == "timeout" then
                table.insert(actions, {
                    type = "timeout_member",
                    user_id = tostring(user.id),
                    seconds = tonumber(ctx.config.raid_timeout_seconds) or 3600,
                    reason = "JoinGuard: ochrona przed rajdem",
                })
            elseif raid_action == "quarantine" and ctx.config.quarantine_role_id and ctx.config.quarantine_role_id ~= "" then
                table.insert(actions, {
                    type = "add_role",
                    user_id = tostring(user.id),
                    role_id = ctx.config.quarantine_role_id,
                    reason = "JoinGuard: kwarantanna w trybie rajdu",
                })
            end
        end

        if #reasons > 0 and ctx.config.alert_channel_id and ctx.config.alert_channel_id ~= "" then
            local prefix = is_raid and "@here 🚨 **Join Guard: ALARM OCHRONY PRZED RAJDEM**\n" or "🛡️ **Join Guard: konto wymaga uwagi**\n"
            table.insert(actions, {
                type = "send_message",
                channel_id = ctx.config.alert_channel_id,
                content = string.format(
                    "%sUżytkownik: %s\nUtworzenie konta: <t:%d:F> (<t:%d:R>)\nAwatar: %s\nPowody: %s",
                    prefix,
                    identity,
                    created_at,
                    created_at,
                    has_avatar == false and "❌ Brak (domyślny)" or "✅ Ustawiony",
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
                    "➡️ **Nowy członek**: %s\nWiek konta: **%d godz. (%d dni)**\nAwatar: %s\nOcena: **%s**",
                    identity,
                    age_hours,
                    age_days,
                    has_avatar == false and "Brak" or "Jest",
                    #reasons > 0 and "wymaga uwagi" or "bez alertów"
                ),
            })
        end

        table.insert(actions, {
            type = "audit",
            event = is_raid and "join_guard_raid_lockdown" or (#reasons > 0 and "join_guard_flagged" or "join_guard_checked"),
            data = {
                user_id = tostring(user.id),
                account_created_at = created_at,
                account_age_hours = age_hours,
                has_avatar = has_avatar,
                recent_joins_count = recent_joins,
                is_raid = is_raid,
                reasons = reasons,
            },
        })

        return actions
    end,
}

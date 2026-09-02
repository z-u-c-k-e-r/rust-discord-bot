local function lower(value)
    return string.lower(value or "")
end

local function escaped(value)
    return zuckerbot.escape_mentions(value or "")
end

local function contains(values, needle)
    needle = tostring(needle or "")
    for _, value in ipairs(values or {}) do
        if tostring(value) == needle then return true end
    end
    return false
end

local function host_allowed(host, allowlist)
    host = lower(host)
    for _, allowed in ipairs(allowlist or {}) do
        allowed = lower(allowed)
        if host == allowed or string.sub(host, -(#allowed + 1)) == "." .. allowed then
            return true
        end
    end
    return false
end

local function matched_domain(content, config)
    for host in string.gmatch(content, "https?://([%w%._%-]+)") do
        host = lower(host)
        if not host_allowed(host, config.allowed_domains) then
            if config.block_all_links == true then
                return host, "all_links"
            end
            for _, blocked in ipairs(config.blocked_domains or {}) do
                blocked = lower(blocked)
                if host == blocked or string.sub(host, -(#blocked + 1)) == "." .. blocked then
                    return host, "blocked_domain"
                end
            end
        end
    end
    return nil, nil
end

local function caps_ratio(content)
    local letters = 0
    local uppercase = 0
    for index = 1, #content do
        local character = string.sub(content, index, index)
        if string.match(character, "%a") then
            letters = letters + 1
            if string.match(character, "%u") then uppercase = uppercase + 1 end
        end
    end
    if letters == 0 then return 0, 0 end
    return uppercase / letters, letters
end

local function longest_repetition(content)
    local longest = 0
    local current = 0
    local previous = nil
    for index = 1, #content do
        local character = string.sub(content, index, index)
        if character == previous then
            current = current + 1
        else
            previous = character
            current = 1
        end
        if current > longest then longest = current end
    end
    return longest
end

local function attachment_violation(attachments, blocked_extensions, max_size)
    for _, attachment in ipairs(attachments or {}) do
        local filename = lower(attachment.filename)
        local extension = string.match(filename, "%.([%w]+)$")
        if extension and contains(blocked_extensions, extension) then
            return "blocked_attachment", filename
        end
        if max_size and max_size > 0 and tonumber(attachment.size or 0) > max_size then
            return "attachment_too_large", filename
        end
    end
    return nil, nil
end

local function evaluate(ctx)
    local config = ctx.config or {}
    local original = ctx.data.content or ""
    local content = lower(original)

    for _, word in ipairs(config.blocked_words or {}) do
        local normalized = lower(word)
        if normalized ~= "" and string.find(content, normalized, 1, true) then
            return "blocked_word", word
        end
    end

    if config.block_invites == true
        and (string.find(content, "discord.gg/", 1, true)
            or string.find(content, "discord.com/invite/", 1, true)
            or string.find(content, "discordapp.com/invite/", 1, true)) then
        return "discord_invite", "invite"
    end

    local host, domain_rule = matched_domain(content, config)
    if host then return domain_rule, host end

    local mention_limit = tonumber(config.max_mentions) or 0
    if mention_limit > 0 and #(ctx.data.mentions or {}) > mention_limit then
        return "mass_mentions", tostring(#(ctx.data.mentions or {}))
    end

    local minimum_letters = tonumber(config.caps_min_letters) or 0
    local maximum_ratio = tonumber(config.max_caps_ratio) or 1
    if minimum_letters > 0 and maximum_ratio < 1 then
        local ratio, letters = caps_ratio(original)
        if letters >= minimum_letters and ratio >= maximum_ratio then
            return "excessive_caps", string.format("%.2f", ratio)
        end
    end

    local repeat_limit = tonumber(config.max_repeated_characters) or 0
    if repeat_limit > 0 and longest_repetition(content) > repeat_limit then
        return "repeated_characters", tostring(longest_repetition(content))
    end

    local attachment_rule, attachment = attachment_violation(
        ctx.data.attachments,
        config.blocked_attachment_extensions or {},
        tonumber(config.max_attachment_size_bytes) or 0
    )
    if attachment_rule then return attachment_rule, attachment end

    return nil, nil
end

local function build_actions(ctx, rule, evidence)
    local config = ctx.config or {}
    local actions = {}

    if config.delete_messages ~= false then
        table.insert(actions, {
            type = "delete_message",
            channel_id = ctx.channel_id,
            message_id = ctx.data.message_id,
        })
    end

    if config.send_notice ~= false then
        local notice = config.notice or "Wiadomość została usunięta przez AutoMod."
        notice = string.gsub(notice, "{user}", "<@" .. tostring(ctx.actor_id) .. ">")
        notice = string.gsub(notice, "{rule}", escaped(rule))
        table.insert(actions, {
            type = "send_message",
            channel_id = ctx.channel_id,
            content = zuckerbot.truncate(notice, 500),
        })
    end

    if config.alert_channel_id and config.alert_channel_id ~= "" then
        table.insert(actions, {
            type = "send_message",
            channel_id = config.alert_channel_id,
            content = string.format(
                "🛡️ **AutoMod zadziałał**\nUżytkownik: <@%s> (`%s`)\nKanał: <#%s>\nReguła: `%s`\nDowód: `%s`\nWiadomość: `%s`",
                tostring(ctx.actor_id),
                tostring(ctx.actor_id),
                tostring(ctx.channel_id),
                escaped(rule),
                escaped(evidence),
                escaped(zuckerbot.truncate(ctx.data.content or "", 500))
            ),
        })
    end

    table.insert(actions, {
        type = "audit",
        event = "automod_rule_match",
        data = {
            message_id = ctx.data.message_id,
            user_id = ctx.actor_id,
            channel_id = ctx.channel_id,
            rule = rule,
            evidence = tostring(evidence or ""),
        },
    })

    return actions
end

return {
    manifest = {
        id = "automod",
        name = "Advanced Lua AutoMod",
        version = "1.0.0",
        description = "Wielowarstwowy filtr treści, linków, zaproszeń, wzmianek, caps locka i załączników.",
        category = "safety",
        default_enabled = false,
        events = {
            "message_create",
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            properties = {
                blocked_words = {
                    type = "array",
                    title = "Zablokowane słowa lub frazy",
                    items = { type = "string", minLength = 1, maxLength = 64 },
                    maxItems = 250,
                },
                blocked_domains = {
                    type = "array",
                    title = "Zablokowane domeny",
                    items = { type = "string", minLength = 3, maxLength = 253 },
                    maxItems = 250,
                },
                allowed_domains = {
                    type = "array",
                    title = "Dozwolone domeny",
                    items = { type = "string", minLength = 3, maxLength = 253 },
                    maxItems = 250,
                },
                block_all_links = { type = "boolean", default = false },
                block_invites = { type = "boolean", default = true },
                max_mentions = { type = "integer", minimum = 0, maximum = 50, default = 5 },
                caps_min_letters = { type = "integer", minimum = 0, maximum = 2000, default = 15 },
                max_caps_ratio = { type = "number", minimum = 0.1, maximum = 1.0, default = 0.8 },
                max_repeated_characters = { type = "integer", minimum = 0, maximum = 100, default = 10 },
                blocked_attachment_extensions = {
                    type = "array",
                    items = { type = "string", minLength = 1, maxLength = 16 },
                    default = { "exe", "scr", "bat", "cmd", "ps1", "vbs" },
                },
                max_attachment_size_bytes = { type = "integer", minimum = 0, default = 0 },
                excluded_channel_ids = {
                    type = "array",
                    items = { type = "string" },
                    maxItems = 100,
                },
                delete_messages = { type = "boolean", default = true },
                send_notice = { type = "boolean", default = true },
                notice = {
                    type = "string",
                    default = "{user}, wiadomość została usunięta przez AutoMod (`{rule}`).",
                    maxLength = 500,
                },
                alert_channel_id = { type = "string", title = "Kanał alertów AutoMod" },
            },
        },
    },

    on_event = function(event, ctx)
        if event ~= "message_create" then return {} end
        if contains(ctx.config.excluded_channel_ids, ctx.channel_id) then return {} end

        local rule, evidence = evaluate(ctx)
        if not rule then return {} end
        return build_actions(ctx, rule, evidence)
    end,
}

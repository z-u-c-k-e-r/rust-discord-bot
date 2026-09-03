local function lower(value)
    return string.lower(value or "")
end

return {
    manifest = {
        id = "automod",
        name = "Lua AutoMod",
        version = "0.1.0",
        description = "Przykładowy filtr słów uruchamiany przez zdarzenie message_create.",
        category = "safety",
        default_enabled = false,
        events = {
            "message_create",
        },
        config_schema = {
            type = "object",
            additionalProperties = false,
            required = { "blocked_words" },
            properties = {
                blocked_words = {
                    type = "array",
                    items = { type = "string", minLength = 2, maxLength = 64 },
                    maxItems = 100,
                },
                notice = {
                    type = "string",
                    default = "Wiadomość została usunięta przez AutoMod.",
                    maxLength = 500,
                },
            },
        },
    },

    on_event = function(event, ctx)
        if event ~= "message_create" then
            return {}
        end

        local content = lower(ctx.data.content)
        for _, word in ipairs(ctx.config.blocked_words or {}) do
            if string.find(content, lower(word), 1, true) then
                return {
                    {
                        type = "delete_message",
                        channel_id = ctx.channel_id,
                        message_id = ctx.data.message_id,
                    },
                    {
                        type = "send_message",
                        channel_id = ctx.channel_id,
                        content = ctx.config.notice or "Wiadomość została usunięta przez AutoMod.",
                    },
                    {
                        type = "audit",
                        event = "automod_word_match",
                        data = {
                            message_id = ctx.data.message_id,
                            user_id = ctx.actor_id,
                            rule = word,
                        },
                    },
                }
            end
        end

        return {}
    end,
}

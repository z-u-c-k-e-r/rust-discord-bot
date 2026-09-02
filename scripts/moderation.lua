local function reason_from(ctx)
  local reason = ctx.options.reason
  if reason == nil or reason == "" then
    return "Brak podanego powodu"
  end
  return reason
end

return {
  manifest = {
    name = "moderation",
    version = "0.1.0",
    description = "Bezpieczne akcje moderacyjne wykonywane przez warstwę Rust.",
    commands = {
      {
        name = "kick",
        description = "Wyrzuca użytkownika z serwera.",
        dm_permission = false,
        required_permissions = {"kick_members"},
        options = {
          {
            name = "user",
            description = "Użytkownik, którego chcesz wyrzucić.",
            kind = "user",
            required = true
          },
          {
            name = "reason",
            description = "Powód widoczny w dzienniku audytu.",
            kind = "string",
            required = false,
            max_length = 400
          }
        }
      },
      {
        name = "ban",
        description = "Banuje użytkownika na serwerze.",
        dm_permission = false,
        required_permissions = {"ban_members"},
        options = {
          {
            name = "user",
            description = "Użytkownik, którego chcesz zbanować.",
            kind = "user",
            required = true
          },
          {
            name = "reason",
            description = "Powód widoczny w dzienniku audytu.",
            kind = "string",
            required = false,
            max_length = 400
          },
          {
            name = "delete_seconds",
            description = "Ile sekund historii wiadomości usunąć (0-604800).",
            kind = "integer",
            required = false,
            min_integer = 0,
            max_integer = 604800
          }
        }
      }
    }
  },

  handle = function(ctx)
    local target = ctx.options.user
    if target == nil then
      return {{ type = "reply", content = "Nie wskazano użytkownika.", ephemeral = true }}
    end

    local reason = reason_from(ctx)
    if ctx.command == "kick" then
      return {
        { type = "kick", user_id = target, reason = reason },
        { type = "reply", content = "✅ Użytkownik został wyrzucony. Powód: " .. reason, ephemeral = true }
      }
    end

    if ctx.command == "ban" then
      return {
        {
          type = "ban",
          user_id = target,
          reason = reason,
          delete_message_seconds = ctx.options.delete_seconds or 0
        },
        { type = "reply", content = "✅ Użytkownik został zbanowany. Powód: " .. reason, ephemeral = true }
      }
    end

    return {{ type = "reply", content = "Nieznana komenda moderacyjna.", ephemeral = true }}
  end
}

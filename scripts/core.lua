return {
  manifest = {
    name = "core",
    version = "0.1.0",
    description = "Podstawowe komendy i informacje o platformie ZuckerBot.",
    commands = {
      {
        name = "ping",
        description = "Sprawdza, czy bot odpowiada."
      },
      {
        name = "bot",
        description = "Pokazuje informacje o architekturze bota."
      },
      {
        name = "help",
        description = "Pokazuje przykładowe dostępne komendy."
      }
    }
  },

  handle = function(ctx)
    if ctx.command == "ping" then
      return {
        {
          type = "reply",
          content = "🏓 Pong! Rdzeń Rust i sandbox Lua działają.",
          ephemeral = true
        }
      }
    end

    if ctx.command == "bot" then
      return {
        {
          type = "reply",
          content = "**ZuckerBot 0.1**\nRust odpowiada za bezpieczeństwo, Discord API, panel i muzykę. Lua deklaruje komendy oraz buduje dozwolone akcje.",
          ephemeral = false
        }
      }
    end

    if ctx.command == "help" then
      return {
        {
          type = "reply",
          content = "**Pierwszy milestone**\n`/ping`, `/bot`, `/coinflip`, `/roll`, `/meme`, `/kick`, `/ban`, `/join`, `/play`, `/pause`, `/resume`, `/skip`, `/stop`, `/leave`\n\nModuły można włączać osobno dla każdego serwera w panelu WWW.",
          ephemeral = true
        }
      }
    end

    return {
      {
        type = "reply",
        content = "Nieznana komenda modułu core.",
        ephemeral = true
      }
    }
  end
}

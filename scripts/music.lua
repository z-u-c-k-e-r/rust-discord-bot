return {
  manifest = {
    name = "music",
    version = "0.1.0",
    description = "Sterowanie kanałem głosowym i kolejką muzyczną Songbird.",
    commands = {
      { name = "join", description = "Dołącza do twojego kanału głosowego.", dm_permission = false },
      {
        name = "play",
        description = "Dodaje utwór lub wyszukiwanie do kolejki.",
        dm_permission = false,
        options = {
          {
            name = "query",
            description = "Adres URL albo tekst wyszukiwania.",
            kind = "string",
            required = true,
            min_length = 1,
            max_length = 500
          }
        }
      },
      { name = "pause", description = "Wstrzymuje odtwarzanie.", dm_permission = false },
      { name = "resume", description = "Wznawia odtwarzanie.", dm_permission = false },
      { name = "skip", description = "Pomija aktualny utwór.", dm_permission = false },
      { name = "stop", description = "Czyści kolejkę i zatrzymuje muzykę.", dm_permission = false },
      { name = "leave", description = "Rozłącza bota z kanału głosowego.", dm_permission = false }
    }
  },

  handle = function(ctx)
    if ctx.command == "join" then
      return {
        { type = "voice_join" },
        { type = "reply", content = "🔊 Dołączono do kanału głosowego.", ephemeral = true }
      }
    end

    if ctx.command == "play" then
      return {
        { type = "music_play", query = ctx.options.query },
        { type = "reply", content = "🎵 Dodano do kolejki: **" .. ctx.options.query .. "**", ephemeral = false }
      }
    end

    if ctx.command == "pause" then
      return {{ type = "music_pause" }, { type = "reply", content = "⏸️ Wstrzymano.", ephemeral = true }}
    end

    if ctx.command == "resume" then
      return {{ type = "music_resume" }, { type = "reply", content = "▶️ Wznowiono.", ephemeral = true }}
    end

    if ctx.command == "skip" then
      return {{ type = "music_skip" }, { type = "reply", content = "⏭️ Pominięto utwór.", ephemeral = true }}
    end

    if ctx.command == "stop" then
      return {{ type = "music_stop" }, { type = "reply", content = "⏹️ Kolejka została wyczyszczona.", ephemeral = true }}
    end

    if ctx.command == "leave" then
      return {{ type = "voice_leave" }, { type = "reply", content = "👋 Rozłączono z kanału głosowego.", ephemeral = true }}
    end

    return {{ type = "reply", content = "Nieznana komenda muzyczna.", ephemeral = true }}
  end
}

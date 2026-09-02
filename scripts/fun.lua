local memes = {
  "Kiedy deploy przechodzi za pierwszym razem: **to na pewno nie produkcja**.",
  "Programista nie śpi. Programista czeka na zakończenie kompilacji Rust.",
  "`works on my machine` — ostatnie słowa przed otwarciem incydentu.",
  "Discord API: 200 OK. Ja: nie dotykamy już niczego.",
  "Lua poprosiła o dostęp do systemu plików. Rust odpowiedział: **nie**."
}

return {
  manifest = {
    name = "fun",
    version = "0.1.0",
    description = "Lekkie komendy społecznościowe, losowania i tekstowe memy.",
    commands = {
      {
        name = "coinflip",
        description = "Rzuca wirtualną monetą."
      },
      {
        name = "roll",
        description = "Losuje liczbę od 1 do wybranej wartości.",
        options = {
          {
            name = "max",
            description = "Najwyższa możliwa liczba (2-100000).",
            kind = "integer",
            required = false,
            min_integer = 2,
            max_integer = 100000
          }
        }
      },
      {
        name = "meme",
        description = "Wyświetla krótki mem programistyczny."
      }
    }
  },

  handle = function(ctx)
    if ctx.command == "coinflip" then
      local side = math.random(0, 1) == 0 and "orzeł" or "reszka"
      return {{ type = "reply", content = "🪙 Wypadł **" .. side .. "**.", ephemeral = false }}
    end

    if ctx.command == "roll" then
      local maximum = ctx.options.max or 100
      local result = math.random(1, maximum)
      return {{ type = "reply", content = "🎲 Wynik: **" .. result .. "** / " .. maximum, ephemeral = false }}
    end

    if ctx.command == "meme" then
      local message = memes[math.random(1, #memes)]
      return {{ type = "reply", content = "😂 " .. message, ephemeral = false }}
    end

    return {{ type = "reply", content = "Nieznana komenda modułu fun.", ephemeral = true }}
  end
}

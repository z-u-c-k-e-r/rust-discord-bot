local memes = {
  "Kiedy kompiluje się za pierwszym razem: sprawdź, czy na pewno uruchomiłeś właściwy projekt.",
  "To nie błąd. To nieudokumentowana funkcja z bardzo krótkim okresem wsparcia.",
  "Programista nie śpi. Programista czeka na zakończenie pipeline'u.",
  "Najpierw działało lokalnie. Potem pojawiła się produkcja.",
  "Jedna mała poprawka później: 37 zmienionych plików."
}

local function stable_index(value)
  local sum = 0
  for index = 1, #value do
    sum = sum + string.byte(value, index)
  end
  return (sum % #memes) + 1
end

return {
  metadata = {
    name = "fun",
    version = "0.1.0",
    description = "Community entertainment commands."
  },
  commands = {
    {
      name = "meme",
      description = "Wyświetla krótki mem programistyczny.",
      dm_permission = true,
      handler = function(ctx)
        return {
          content = memes[stable_index(ctx.interaction_id)],
          ephemeral = false
        }
      end
    }
  }
}

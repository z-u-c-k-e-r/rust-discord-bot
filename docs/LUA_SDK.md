# ZuckerBot Lua SDK

## Wersja kontraktu

Aktualny kontrakt: `0.1`.

Runtime używa Lua 5.4 przez `mlua`. Każdy plugin ma własny entrypoint `plugins/<plugin-id>/main.lua` i zwraca tabelę z metadanymi oraz handlerami.

## Minimalny plugin

```lua
return {
  metadata = {
    name = "example",
    version = "0.1.0",
    description = "Example plugin"
  },
  commands = {
    {
      name = "hello",
      description = "Wita użytkownika.",
      dm_permission = true,
      handler = function(ctx)
        return {
          content = "Cześć, " .. ctx.user_name,
          ephemeral = false
        }
      end
    }
  }
}
```

## Metadane

| Pole | Typ | Ograniczenie |
|---|---|---|
| `name` | string | 1–64, małe litery ASCII, cyfry, `-`, `_` |
| `version` | string | 1–32 |
| `description` | string | 1–200 |

Nazwa pluginu musi być unikalna w danym runtime.

## Komenda

| Pole | Typ | Wymagane | Opis |
|---|---|---:|---|
| `name` | string | tak | Unikalna nazwa komendy Discord, 1–32 znaki |
| `description` | string | tak | Opis 1–100 znaków |
| `dm_permission` | boolean | nie | Czy komenda może działać w DM; domyślnie `true` |
| `handler` | function | tak | Funkcja otrzymująca kontekst i zwracająca odpowiedź |

Obecna wersja rejestruje wyłącznie komendy bez parametrów. Typed options, subcommands, autocomplete, context commands i lokalizacje są kolejnym rozszerzeniem kontraktu.

## `CommandContext`

```lua
{
  command_name = "hello",
  interaction_id = "...",
  user_id = "...",
  user_name = "Adrian",
  guild_id = "...",       -- nil w DM
  channel_id = "...",
  locale = "pl",
  options = { ... }
}
```

Identyfikatory Discorda są stringami. Nie konwertuj snowflake do `number`, ponieważ liczby Lua nie zachowują precyzji wszystkich 64-bitowych identyfikatorów.

## Odpowiedź

```lua
{
  content = "Treść do 2000 znaków Unicode",
  ephemeral = false
}
```

Pusta odpowiedź i treść przekraczająca limit Discorda są odrzucane przez Rust.

## Sandbox

Każde wykonanie ma limit pamięci i instrukcji. Runtime nie udostępnia:

- `os`;
- `io`;
- `package` i `require`;
- `dofile` i `loadfile`;
- dynamicznego `load`;
- `debug`;
- `collectgarbage`;
- tokenu Discorda;
- bezpośredniego systemu plików, procesu, powłoki lub sieci.

Limit chroni proces przed nieskończonymi pętlami i nadmierną alokacją, ale nie zastępuje przeglądu kodu. W obecnym etapie lokalne pliki traktujemy jako zaufane źródło utrzymywane w repozytorium.

## Planowane API capabilities

Poniższe API nie jest jeszcze zaimplementowane. Pokazuje docelowy kierunek bez obiecywania niebezpiecznego pełnego dostępu:

```lua
local plugin = require("zuckerbot")

plugin.command({ ... })
plugin.on("message_create", function(event) ... end)
plugin.storage:get("key")
plugin.storage:set("key", value)
plugin.schedule:after("10m", "job-name", payload)
plugin.discord:send_message(channel_id, message)
plugin.http:request("named-allowlisted-service", request)
```

Rust będzie sprawdzał capability, konfigurację serwera, uprawnienia aktora, hierarchię ról, rate limit i limit rozmiaru danych. Każda operacja administracyjna otrzyma wpis audytowy.

## Wersjonowanie

- zmiany zgodne wstecznie zwiększają minor wersji SDK;
- zmiany łamiące wymagają nowej major wersji kontraktu;
- plugin deklaruje zakres obsługiwanych wersji;
- migracje konfiguracji i storage pluginu będą transakcyjne;
- przyszły hot reload najpierw waliduje nową wersję, a dopiero potem atomowo podmienia runtime.

## Testowanie

Testy Rust mogą ładować plugin ze stringa przez `LuaRuntime::load_plugin_source`. Docelowo powstanie `zuckerbot plugin test` z fixture zdarzeń, snapshotami odpowiedzi, symulacją capabilities i kontrolowanym zegarem.

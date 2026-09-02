# Status projektu

## Bieżący etap

`platform-foundation` — pierwszy uruchamialny przekrój Rust + Lua + Discord + WWW.

## Dostarczone w gałęzi `feat/platform-foundation`

- workspace Rust z oddzielonym core, config, Lua runtime, botem i API;
- Serenity 0.12.5 i Songbird 0.6.0;
- automatyczne wykrywanie `plugins/*/main.lua`;
- komendy Lua `/ping`, `/about`, `/meme`;
- limit pamięci i instrukcji oraz usunięte niebezpieczne globalne biblioteki Lua;
- synchronizacja komend globalnie lub na guild deweloperskim;
- panel startowy oraz endpointy health/meta;
- PostgreSQL/Redis Compose i początkowa migracja;
- Dockerfile, CI, dokumentacja architektury, bezpieczeństwa i pełnej macierzy funkcji.

## Jeszcze niegotowe

- logowanie Discord OAuth2 i rzeczywista konfiguracja z panelu;
- połączenie aplikacji z PostgreSQL/Redis;
- komendy z parametrami, event handlers i capability broker Lua;
- kompletna moderacja, AutoMod, tickety, role, poziomy i integracje;
- kolejka muzyczna oraz provider audio;
- hot reload i izolacja niezaufanych pluginów;
- metryki produkcyjne, workery i system wdrożeń.

## Gate jakości

Pierwszy commit implementacyjny oczekuje na pełne CI. Po pierwszym zielonym przebiegu ten plik i `WORKLOG.md` zostaną uzupełnione dokładnym SHA, a następnie wymagany będzie drugi zielony przebieg dla finalnego SHA PR.

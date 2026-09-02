# Środowisko deweloperskie

## Wymagania

- Rust 1.88.0, `rustfmt`, `clippy`;
- CMake i `pkg-config`;
- Opus development headers;
- Docker z Compose do PostgreSQL i Redis;
- Node.js tylko do statycznej kontroli JavaScript;
- FFmpeg do późniejszych testów audio.

Ubuntu/Debian:

```bash
sudo apt-get update
sudo apt-get install -y build-essential cmake pkg-config libopus-dev ffmpeg
```

## Konfiguracja aplikacji Discord

1. Utwórz aplikację i użytkownika bota w Discord Developer Portal.
2. Skopiuj `.env.example` do `.env`.
3. Ustaw `DISCORD_TOKEN`.
4. Ustaw `DISCORD_DEVELOPMENT_GUILD_ID`, aby komendy deweloperskie aktualizowały się natychmiast.
5. Dodaj bota ze scope `bot` oraz `applications.commands`.
6. Nie włączaj privileged intents, dopóki konkretny moduł ich nie potrzebuje.
7. Dla przyszłej muzyki bot potrzebuje `Connect` i `Speak` na odpowiednich kanałach.

## Komendy jakości

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
node --check apps/api/static/app.js
docker compose config
```

## Uruchomienie

```bash
cargo run -p zuckerbot-api
cargo run -p zuckerbot-bot
```

## Konwencje

- nie używamy `unwrap()` ani `expect()` w ścieżkach produkcyjnych, chyba że proces nie może bezpiecznie kontynuować po błędzie inicjalizacji systemowej;
- błędy dla użytkownika nie ujawniają sekretów ani stosu;
- identyfikatory Discorda pozostają stringami w Lua i bazie;
- każda nowa uprzywilejowana operacja wymaga modelu capability i testów odmowy;
- długie zadania nie blokują event handlera Gateway;
- konfiguracja ma bezpieczne wartości domyślne;
- frontend nie przechowuje tokenu bota;
- WORKLOG jest append-only.

## Pull request

PR powinien opisywać problem, decyzję, wszystkie zmienione obszary, testy, ryzyka, plan rollbacku i wpływ na bezpieczeństwo. Merge jest dopuszczalny dopiero po pełnym zielonym CI dla dokładnego SHA głowy PR.

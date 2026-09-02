# P3 Handoff — platform foundation

## Punkt przekazania

- Repozytorium: `z-u-c-k-e-r/rust-discord-bot`
- Pull request: `#1 feat: bootstrap ZuckerBot platform foundation`
- Zweryfikowany commit implementacyjny: `e9ad0cadbfe20e62450fe61a52fc82fe4f89be78`
- Zielony workflow: `CI #2`, run `33663086930`
- Zakres kontroli: exact-head checkout, rustfmt, Clippy `-D warnings`, testy, build, JavaScript, Docker Compose, smoke test API i artefakt `Cargo.lock`

Commit zawierający ten handoff musi przejść drugi pełny przebieg CI. Nie należy uznawać PR za gotowy na podstawie starszego SHA.

## Stabilne kontrakty obecnego etapu

### Plugin Lua

- entrypoint: `plugins/<plugin-id>/main.lua`;
- plugin zwraca `metadata` i tablicę `commands`;
- handler otrzymuje `CommandContext` bez sekretów;
- handler zwraca `CommandResponse` z `content` i `ephemeral`;
- nazwy, opisy i wynik są walidowane po stronie Rust;
- runtime ma limit pamięci i instrukcji;
- brak `os`, `io`, `package`, `require`, `debug`, dynamicznego ładowania i dostępu do sieci.

### Discord

- komendy są synchronizowane jako zestaw globalny albo guild deweloperski;
- privileged intents są wyłączone domyślnie;
- proces używa automatycznego shardingu;
- warstwa voice jest zarejestrowana przez Songbird, ale nie ma jeszcze kolejki audio.

### Control API

- `GET /health` zwraca stan, wersję i uptime;
- `GET /api/v1/meta` opisuje bieżący etap i moduły;
- statyczny panel jest osadzony w binarce;
- OAuth2, sesje, RBAC i trwała konfiguracja nie są jeszcze zaimplementowane.

## Następny kamień milowy: control plane i trwała konfiguracja

### Tor danych

1. Dodać SQLx z pulą PostgreSQL i migracjami uruchamianymi w kontrolowany sposób.
2. Dodać klienta Redis, readiness checks i jawne timeouty.
3. Utworzyć repository traits dla guild, module configuration, audit i session.
4. Oddzielić dane domenowe od modeli SQLx.
5. Dodać testy integracyjne z usługami w CI.

### Tor OAuth2 i RBAC

1. Zaimplementować Discord Authorization Code Grant ze `state` i bezpiecznymi cookies.
2. Użyć minimalnych scopes potrzebnych do identyfikacji użytkownika i listy serwerów.
3. Zweryfikować bieżące uprawnienia zarządzania serwerem przed każdym zapisem.
4. Wprowadzić role panelu: owner, administrator, moderator, support i viewer.
5. Zapisać każdą zmianę konfiguracji w `audit_events` z request ID i diffem.

### Tor registry modułów

1. Wersjonowany katalog modułów i zależności.
2. Enable/disable per guild z bezpiecznymi defaults.
3. Schemat konfiguracji i migracje wersji.
4. Capability manifest pluginu.
5. Atomiczny reload konfiguracji bez restartu Gateway.

### Tor panelu

1. Logowanie i wybór serwera.
2. Layout serwera, nawigacja modułów i stany loading/error/empty.
3. Formularze generowane ze schematu z walidacją po obu stronach.
4. Preview diffu przed zapisem.
5. Widok audytu i sesji.

## Kryteria akceptacji kolejnego kamienia milowego

- użytkownik loguje się przez Discord bez ujawnienia tokenu bota;
- panel pokazuje wyłącznie serwery, którymi użytkownik może zarządzać;
- zapis konfiguracji jest autoryzowany, walidowany, transakcyjny i audytowany;
- bot odczytuje zmianę bez restartu;
- health i readiness rozróżniają API, PostgreSQL i Redis;
- wszystkie operacje mają test sukcesu, odmowy i błędu zależności;
- pełne CI jest zielone dla dokładnego finalnego SHA.

## Ryzyka do pilnowania

- nie wdrażać edytora niezaufanego Lua przed capability brokerem i izolacją tenantów;
- nie przechowywać tokenów OAuth lub sekretów integracji jawnie w JSONB;
- nie trzymać połączenia z bazą ani operacji sieciowej podczas oczekiwania na Discord interaction response;
- nie wykonywać długich zadań na wątku handlera Gateway;
- nie zakładać, że uprawnienia użytkownika nie zmieniły się od czasu logowania;
- nie kodować na sztywno limitów Discorda zamiast reagować na odpowiedzi API.

## Komendy weryfikacyjne

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
node --check apps/api/static/app.js
cp .env.example .env
docker compose config --quiet
```

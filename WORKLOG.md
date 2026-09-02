# WORKLOG

Dziennik jest append-only. Nowe wpisy dodajemy na końcu; nie poprawiamy historii przez przepisywanie starszych wpisów.

## 2026-09-02 — rozpoczęcie projektu

### Problem

Repozytorium nie miało implementacji. Celem jest zaawansowana platforma Discord przewyższająca typowe boty all-in-one, z rdzeniem w Rust, modułami Lua, muzyką i zarządzaniem przez WWW.

### Decyzje

- Rust pozostaje granicą zaufania dla Discorda, uprawnień, sekretów, storage i voice.
- Lua 5.4 odpowiada za rozszerzenia, ale działa z limitem pamięci i instrukcji, bez OS/IO/network.
- Komendy są rejestrowane dynamicznie bez Serenity Standard Framework.
- Control plane jest osobnym procesem Axum.
- PostgreSQL jest źródłem prawdy, Redis warstwą koordynacji i cache.
- Funkcje są rozwijane modułowo według macierzy i roadmapy, zamiast jako jeden monolit.

### Zmiany

- dodano workspace i pięć pakietów Rust;
- dodano klienta Discord i rejestrację Songbird;
- dodano loader/sandbox Lua oraz testy;
- dodano pierwsze trzy komendy Lua;
- dodano API, panel startowy, Docker, Compose i migrację;
- dodano dokumentację produktu, architektury i bezpieczeństwa.

### Walidacja

Oczekuje na pierwszy przebieg CI dla commita implementacyjnego.

### Ryzyka i następne kroki

- API Lua jest celowo minimalne; przed edytorem WWW potrzebne są capabilities i silniejsza izolacja tenantów.
- Voice manager nie jest jeszcze odtwarzaczem; kolejka i providery powstaną po trwałej konfiguracji.
- Pierwszym kolejnym etapem powinny być OAuth2/RBAC, SQLx/Redis oraz wersjonowany registry modułów.

## 2026-09-02 — pierwszy pełny zielony przebieg

### Wykryte problemy

Pierwszy run `33662598113` zatrzymał się na `rustfmt`, zanim rozpoczęła się kompilacja. Log wskazał wszystkie wymagane zmiany formatowania. Dodatkowo workflow pull requestu domyślnie checkoutował tymczasowy merge commit GitHuba, co nie spełniało wymogu kontroli dokładnego SHA głowy.

### Poprawki

- zastosowano dokładny wynik `cargo fmt` dla wszystkich wskazanych plików;
- checkout ustawiono na `github.event.pull_request.head.sha` z fallbackiem dla push;
- dodano osobny krok porównujący `git rev-parse HEAD` z oczekiwanym SHA;
- walidacja Compose tworzy lokalny `.env` z bezpiecznego przykładu;
- zachowano generowany `Cargo.lock` jako artefakt workflow.

### Zweryfikowany wynik

Commit `e9ad0cadbfe20e62450fe61a52fc82fe4f89be78` przeszedł pełny workflow `CI #2`, run `33663086930`:

- exact-head checkout i jawna weryfikacja SHA;
- rustfmt;
- Clippy dla całego workspace i wszystkich targetów z `-D warnings`;
- testy wszystkich pakietów;
- pełny build workspace;
- kontrola JavaScript panelu;
- walidacja Docker Compose;
- uruchomienie API i smoke test `/health`;
- publikacja artefaktu `Cargo.lock` o digest `sha256:fbdbab2a1e6e3ebd3f117fdf2803f2fc261a5e4364a11e61a2e24cd5e459f3fd`.

### Closeout

Po pierwszym zielonym runie dodano P3 handoff, datowaną analizę oficjalnego Discord API, aktualny status oraz test wczytujący rzeczywiste pluginy `core` i `fun`. Commit closeout wymaga drugiego pełnego zielonego przebiegu dla dokładnego finalnego SHA.

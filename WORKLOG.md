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

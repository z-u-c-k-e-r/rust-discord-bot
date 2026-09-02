# PostgreSQL, Redis i kontrakt gotowości

## Cel

Ten etap wprowadza pierwszą trwałą warstwę control plane. Aplikacja nadal nie udostępnia zapisu konfiguracji przez publiczne HTTP, ponieważ logowanie Discord OAuth2 i RBAC są następną zależnością. Warstwa danych jest jednak gotowa do bezpiecznego użycia przez autoryzowane endpointy.

## Rozdzielenie liveness i readiness

`GET /health` odpowiada na pytanie, czy proces API działa i może obsłużyć HTTP. Nie wykonuje zapytań do bazy ani Redis, dlatego może służyć jako liveness probe bez wywoływania pętli restartów podczas krótkiej awarii zależności.

`GET /ready` sprawdza równolegle:

- `SELECT 1` przez pulę PostgreSQL;
- `PING` przez asynchroniczne połączenie Redis.

Każde sprawdzenie ma osobny limit czasu. Odpowiedź ma kod `200`, gdy obie zależności są dostępne, albo `503`, gdy co najmniej jedna jest niedostępna lub przekroczyła timeout.

```json
{
  "status": "ready",
  "postgres": { "state": "ok", "latency_ms": 2 },
  "redis": { "state": "ok", "latency_ms": 1 }
}
```

Odpowiedź nie zawiera URL połączeń, nazw użytkowników, haseł ani surowych komunikatów sterowników.

## Konfiguracja

```dotenv
DATABASE_URL=postgres://zuckerbot:zuckerbot@localhost:5432/zuckerbot
REDIS_URL=redis://localhost:6379
DATABASE_MAX_CONNECTIONS=10
DATABASE_ACQUIRE_TIMEOUT_MS=3000
DEPENDENCY_TIMEOUT_MS=1500
RUN_DATABASE_MIGRATIONS=false
```

`RUN_DATABASE_MIGRATIONS` jest jawne. W Docker Compose może być włączone, natomiast wdrożenie produkcyjne może wykonywać migracje jako osobny kontrolowany job przed rolloutem.

Typy konfiguracji mają ręczne implementacje `Debug`, które redagują token Discorda, `DATABASE_URL` i `REDIS_URL`.

## Repository contracts

Crate `zuckerbot-storage` udostępnia pierwsze kontrakty:

- `GuildRepository`;
- `ModuleConfigurationRepository`.

Adapter PostgreSQL implementuje oba interfejsy. Discord snowflake pozostaje stringiem i przechodzi walidację od 1 do 20 cyfr ASCII, dzięki czemu nie tracimy precyzji w JavaScript lub Lua.

## Optymistyczna kontrola współbieżności

Każda konfiguracja modułu posiada dodatnią wersję `BIGINT`.

- `expected_version = 0` oznacza próbę utworzenia nowej konfiguracji;
- dla istniejącego rekordu klient przekazuje wersję, którą wcześniej odczytał;
- `UPDATE` wykonuje się wyłącznie, gdy wersja nadal pasuje;
- sukces zwiększa wersję o jeden;
- niezgodność zwraca `StorageError::VersionConflict`, bez nadpisania cudzej zmiany.

Dzięki temu dwóch administratorów nie zapisze po cichu sprzecznych ustawień. Przyszły panel pokaże konflikt i diff zamiast stosować strategię „ostatni zapis wygrywa”.

## Limity wejścia

- identyfikator modułu: do 64 znaków, małe litery ASCII, cyfry, `-`, `_`;
- locale: do 16 znaków;
- nazwa serwera: do 100 znaków;
- konfiguracja JSON: maksymalnie 64 KiB;
- `expected_version` nie może być ujemne;
- `updated_by` musi być jawne.

## Migracje i sesje

`0002_control_plane.sql` dodaje wersje rekordów, wynik/kod błędu audytu i tabelę serwerowych sesji WWW. Tabela przechowuje hash identyfikatora sesji i hash CSRF, a nie token sesji w postaci jawnej.

## Testy

CI uruchamia prawdziwe PostgreSQL 17 i Redis 8, po czym sprawdza migracje, readiness, utworzenie serwera, zapis wersji 1, aktualizację do wersji 2, odmowę starej wersji oraz ponowny odczyt rekordu. Osobny smoke test uruchamia API i sprawdza `/health` oraz `/ready`.

## Następny krok

Na tym kontrakcie powstaną Discord OAuth2, sesje serwerowe, CSRF, guild selector, rewalidacja uprawnień, RBAC oraz autoryzowane endpointy konfiguracji z pełnym audytem zmian.

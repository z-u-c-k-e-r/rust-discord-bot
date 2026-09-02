# Architektura ZuckerBot

## Cele

ZuckerBot ma być platformą, a nie zbiorem przypadkowych komend. Architektura musi jednocześnie zapewnić:

1. niskie opóźnienia i stabilność rdzenia;
2. rozszerzalność bez rekompilacji całego bota;
3. bezpieczną izolację konfiguracji i skryptów wielu serwerów;
4. pełny audyt działań administracyjnych;
5. możliwość skalowania od jednego procesu do wielu shardów i workerów;
6. jednolity panel WWW dla właścicieli, administratorów i moderatorów;
7. odporność na błędy integracji zewnętrznych i ograniczenia Discord API.

## Główne procesy

### `zuckerbot-bot`

Proces czasu rzeczywistego odpowiedzialny za Discord Gateway, interakcje aplikacji oraz integrację głosową. Nie powinien wykonywać długich zadań w ścieżce obsługi zdarzenia. Cięższa praca trafia docelowo do kolejki workerów.

### `zuckerbot-api`

Control plane oparty na Axum. Docelowo obsłuży Discord OAuth2, wybór serwera, RBAC, konfigurację modułów, edytor Lua, podgląd audytu, analitykę i bezpieczne testowanie automatyzacji.

### Workery

Planowana grupa procesów do harmonogramów, powiadomień, generowania obrazów powitalnych, transcriptów ticketów, importów/eksportów, integracji zewnętrznych oraz zadań AI. Zadania muszą być idempotentne, mieć retry z backoffem i kolejkę błędów.

## Warstwy kodu

- `crates/core`: stabilne kontrakty domenowe niezależne od Discorda i Lua;
- `crates/config`: walidowana konfiguracja bez wyświetlania sekretów;
- `crates/lua-runtime`: ładowanie, walidacja, sandbox i wykonywanie skryptów;
- `apps/bot`: adapter Discorda i voice manager;
- `apps/api`: HTTP API i panel sterowania;
- `plugins`: wersjonowane moduły Lua;
- `migrations`: trwały model danych PostgreSQL.

## Przepływ komendy

1. Discord wysyła interakcję do właściwego sharda.
2. Serenity deserializuje zdarzenie.
3. Rust buduje minimalny `CommandContext`, bez tokenów i sekretów.
4. Rejestr znajduje handler Lua.
5. Sandbox resetuje budżet instrukcji i wykonuje funkcję.
6. Wynik jest deserializowany do `CommandResponse` i walidowany.
7. Rust wysyła odpowiedź, respektując rate limit Discorda.
8. Docelowo metryka czasu wykonania i wynik trafią do telemetryki.

## Model rozszerzeń Lua

Obecny etap implementuje komendy i odpowiedzi tekstowe. Docelowy model jest oparty na capabilities:

- `discord.messages.send`;
- `discord.messages.manage`;
- `discord.members.moderate`;
- `discord.roles.manage`;
- `discord.voice.control`;
- `storage.guild.read` / `storage.guild.write`;
- `scheduler.create`;
- `http.allowlisted`;
- `secrets.named.read` bez ujawniania wartości skryptowi, gdy możliwe jest wykonanie żądania po stronie Rust.

Moduł deklaruje wymagane capabilities w manifeście. Instalujący administrator widzi zakres i zatwierdza go. Każde wywołanie jest dodatkowo ograniczone rzeczywistymi uprawnieniami bota, użytkownika i hierarchią ról.

## Dane

PostgreSQL przechowuje stan trwały i audyt. Snowflake Discorda są zapisywane jako tekst, żeby nie zależeć od ograniczeń signed `BIGINT` w innych systemach. Konfiguracje modułów mają wersjonowane schematy JSONB, ale krytyczne dane operacyjne dostają jawne kolumny i indeksy.

Redis będzie używany do:

- cache krótkotrwałego;
- rozproszonych rate limitów;
- blokad i koordynacji shardów;
- kolejek oraz opóźnionych zadań;
- deduplikacji zdarzeń i kluczy idempotencji;
- szybkich leaderboardów.

Sekrety nie będą przechowywane jawnie w konfiguracjach JSONB. Docelowo wymagany jest zaszyfrowany secret store z rotacją kluczy i rozdzieleniem dostępu.

## Skalowanie

Pierwsza wersja może działać jako jeden proces bota i jeden proces API. Granice modułów są przygotowane pod:

- automatyczny sharding Gateway;
- routing serwera do właściciela sharda;
- wiele replik API za load balancerem;
- osobne pule workerów zależnie od rodzaju zadań;
- stateless API z sesjami w Redis;
- partycjonowanie tabel zdarzeń i audytu;
- graceful shutdown i ponowne przejęcie zadań.

## Niezawodność

Każda integracja zewnętrzna powinna mieć timeout, ograniczoną liczbę prób, backoff z jitterem, circuit breaker oraz metryki. Operacje masowe wymagają limitu, dry-run i raportu częściowych błędów. Zdarzenia Discorda mogą zostać dostarczone ponownie, dlatego przetwarzanie musi być odporne na duplikaty.

## Granice obecnego etapu

Fundament rejestruje komendy z lokalnych, zaufanych plików Lua. Nie ma jeszcze hot reloadu, per-guild pluginów, capability brokerów, OAuth2, trwałego storage w runtime ani produkcyjnej kolejki muzyki. Te ograniczenia są jawne w `STATUS.md` i roadmapie.

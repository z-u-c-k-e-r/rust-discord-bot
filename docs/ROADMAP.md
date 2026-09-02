# Roadmapa ZuckerBot

Roadmapa jest uporządkowana według zależności i ryzyka. Priorytetem nie jest liczba komend, lecz bezpieczny fundament, dzięki któremu kolejne moduły można rozwijać równolegle.

## Etap 0 — fundament platformy

**Cel:** uruchamialny szkielet Rust + Lua + WWW.

- workspace i granice crate/app;
- Serenity Gateway i synchronizacja komend;
- Songbird voice foundation;
- sandbox Lua z pamięcią, instrukcjami i walidacją;
- przykładowe moduły core/fun;
- control API i panel startowy;
- PostgreSQL/Redis w Compose;
- pierwsza migracja, CI i dokumentacja.

**Stan:** realizowany w pierwszym PR.

## Etap 1 — control plane i trwała konfiguracja

- SQLx connection pools i migracje przy wdrożeniu;
- Redis client i health/readiness;
- Discord OAuth2, bezpieczne sesje i guild selector;
- RBAC panelu oraz audyt każdej zmiany;
- registry modułów, enable/disable per guild;
- wersjonowane schema konfiguracji;
- secret store abstraction;
- command option schema i lokalizacje;
- reload konfiguracji bez restartu.

**Kryterium wyjścia:** administrator loguje się przez Discord, wybiera serwer, zmienia bezpieczną konfigurację i widzi audyt; bot stosuje zmianę.

## Etap 2 — moderacja, AutoMod i logowanie

- case system, warn/timeout/kick/ban/softban;
- purge, slowmode i odwracalny lockdown;
- hierarchia ról, dry-run i partial failure reports;
- logi wiadomości, memberów, ról, kanałów i voice;
- filtry spam/mentions/links/keywords/regex;
- native AutoMod synchronization;
- verification i raid mode;
- modmail i appeals foundation.

**Kryterium wyjścia:** kompletna, audytowana ścieżka od reguły lub komendy do sprawy, działania, powiadomienia i ewentualnego cofnięcia.

## Etap 3 — role, onboarding, ticketing i automatyzacje

- autorole, button/select roles, temporary/persistent roles;
- welcome/leave i onboarding profiles;
- temporary voice channels;
- ticket panels, modals, claim/transfer/SLA/transcripts;
- visual workflow engine: trigger → conditions → actions;
- scheduler, recurring jobs i reminders;
- plugin capabilities, scoped storage i Discord action broker.

**Kryterium wyjścia:** właściciel buduje typowy workflow z panelu bez kodu, a zaawansowany administrator może stworzyć równoważny plugin Lua.

## Etap 4 — muzyka i system voice

- voice state machine i reconnect/failover;
- kolejka, kontrolki, DJ role i vote skip;
- provider abstraction z legalnymi źródłami;
- playlisty, favorites, autoplay i streams;
- DSP z limitami CPU;
- metryki jakości i multi-node readiness;
- opt-in recording/transcription dopiero po pełnym modelu zgód.

**Kryterium wyjścia:** stabilne odtwarzanie, kontrolowana kolejka, odporność na błędy providera i jasna zgodność z regulaminami źródeł.

## Etap 5 — społeczność, poziomy i ekonomia

- XP text/voice, rank cards, role rewards i seasons;
- reputation i leaderboardy;
- ledger-based economy, shop, inventory, daily/streak;
- starboard, suggestions, polls, trivia, counting i minigry;
- birthdays, partnerships, events i giveaways;
- anti-farm, fraud controls i admin audit.

## Etap 6 — integracje i analityka

- RSS, YouTube, Twitch, GitHub i generic signed webhooks;
- named HTTP connectors i provider health;
- dashboardy wzrostu, retencji, aktywności i moderacji;
- Prometheus/OpenTelemetry, tracing i alerty;
- import/export, backup/restore i polityki retencji.

## Etap 7 — ekosystem pluginów i opcjonalne AI

- CLI SDK, simulator, fixtures i plugin packaging;
- podpisy, review, marketplace i bezpieczne aktualizacje;
- per-tenant runtime isolation i quotas;
- AI provider abstraction, cost controls i data controls;
- Q&A, summaries, translation i ticket assistance;
- moderation assistance wyłącznie z zasadami, audytem i kontrolą człowieka.

## Praca równoległa agentów

Po ustabilizowaniu Etapu 1 praca może zostać podzielona na niezależne tory:

- **Gateway/Discord:** interakcje, event routing, permissions, sharding;
- **Lua platform:** SDK, capabilities, isolation, tests, hot reload;
- **Control API:** OAuth2, RBAC, configuration, audit;
- **Frontend:** design system, forms, workflow builder, analytics;
- **Moderation/Safety:** case engine, rules, actions, appeals;
- **Voice/Music:** state machine, queue, providers, nodes;
- **Data/Workers:** schemas, queues, jobs, retention, backups;
- **Quality/Security:** threat modeling, CI, fuzzing, load tests, release gates.

Każdy tor musi pracować na jawnych kontraktach. Zmiana kontraktu wymaga aktualizacji dokumentacji i testów integracyjnych, zanim inni agenci oprą na niej kod.

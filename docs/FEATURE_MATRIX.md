# Macierz funkcji ZuckerBot

Legenda:

- **Gotowe** — działa w aktualnym kodzie i ma testy lub ścieżkę uruchomieniową;
- **Fundament** — istnieje kontrakt, infrastruktura albo UI, ale nie cały moduł użytkowy;
- **Plan** — zaakceptowany zakres produktu, jeszcze bez implementacji;
- **Ograniczone** — funkcja będzie dostępna wyłącznie z dodatkowymi zabezpieczeniami, zgodą lub zgodnie z API dostawcy.

Ta macierz opisuje docelową platformę. Nie oznacza, że wszystkie pozycje są już wdrożone.

## 1. Platforma Discord i interakcje

| Funkcja | Stan | Zakres |
|---|---|---|
| Gateway i REST | Gotowe | Serenity, automatyczny sharding, reconnect i obsługa interakcji |
| Komendy slash z Lua | Gotowe | Dynamiczne wykrywanie, walidacja i synchronizacja |
| Serwer deweloperski | Gotowe | Natychmiastowa rejestracja komend przez guild ID |
| Komendy globalne | Gotowe | Zbiorcza synchronizacja komend aplikacji |
| Parametry i subkomendy | Plan | String, integer, number, boolean, user, role, channel, mentionable, attachment |
| Autocomplete | Plan | Sugestie generowane przez Rust lub Lua z limitem czasu |
| User context commands | Plan | Akcje z menu kontekstowego użytkownika |
| Message context commands | Plan | Akcje z menu kontekstowego wiadomości |
| Przyciski i select menus | Plan | String/user/role/mentionable/channel selects |
| Modale i formularze | Plan | Walidowane formularze dla ticketów, konfiguracji i workflow |
| Embedy, pliki i załączniki | Plan | Szablony, ograniczenia rozmiaru, skan typu zawartości |
| Polls | Plan | Natywne ankiety Discorda i warianty zaawansowane |
| Threads i forum channels | Plan | Tworzenie, tagowanie, archiwizacja, automatyzacja |
| Scheduled events | Plan | Tworzenie, przypomnienia, RSVP i synchronizacja |
| Webhooks | Plan | Zarządzane webhooki z audytem i bez ujawniania tokenów |
| Lokalizacje | Plan | Nazwy/opisy komend i odpowiedzi zależne od locale |
| Installation contexts | Plan | Guild install i bezpieczne user install tam, gdzie pasuje |
| Uprawnienia komend | Plan | Domyślne permissions, RBAC panelu i reguły per kanał/rola |

## 2. Moderacja i system spraw

| Funkcja | Stan | Zakres |
|---|---|---|
| Schemat spraw moderacyjnych | Fundament | Case number, typ akcji, cel, moderator, powód, dowody, wygaśnięcie i cofnięcie |
| Warn / note | Plan | Ostrzeżenia, notatki prywatne, progi eskalacji |
| Timeout /untimeout | Plan | Czasowe ograniczenie z harmonogramem i audytem |
| Kick | Plan | Kontrola uprawnień i hierarchii |
| Ban / unban / softban | Plan | Ban czasowy, usuwanie wiadomości, odwołania |
| Mute role fallback | Plan | Tylko dla serwerów wymagających starszego modelu |
| Purge | Plan | Filtry autora, typu, wieku, treści, załączników i botów |
| Slowmode | Plan | Ustawienie ręczne i automatyczne podczas spamu |
| Lockdown | Plan | Kanał, kategoria lub serwer; snapshot i bezpieczny rollback |
| Nickname management | Plan | Zmiana/reset z hierarchią ról |
| Role management | Plan | Add/remove, masowe akcje z limitem i dry-run |
| Channel management | Plan | Create/edit/archive z szablonami i audytem |
| Voice moderation | Plan | Move, disconnect, mute/deafen z kontrolą uprawnień |
| Mass actions | Ograniczone | Potwierdzenie, limit, dry-run, partial failure report i kill switch |
| Dowody i załączniki | Plan | Snapshot treści, linki, pliki, hashe i polityka retencji |
| Historia użytkownika | Plan | Chronologia spraw, filtry i eksport |
| Punkty karne | Plan | Konfigurowalne progi i automatyczna eskalacja |
| Appeals | Plan | Formularze, kolejka, decyzje i komunikacja |
| Modmail | Plan | Wątki prywatne, anonimizacja moderatora opcjonalna, transcript |
| Audit log Discorda | Plan | Korelacja akcji bota i zmian natywnych |

## 3. AutoMod, antyspam i bezpieczeństwo serwera

| Funkcja | Stan | Zakres |
|---|---|---|
| Słowa/wyrażenia | Plan | Exact, wildcard, normalizacja, wyjątki i allowlisty |
| Regex | Plan | Bezpieczny silnik, limit czasu i walidacja ReDoS |
| Filtry presetów | Plan | Obelgi, treści seksualne, slurs zależnie od ustawień serwera |
| Linki i domeny | Plan | Allowlist/blocklist, skracacze, redirect inspection |
| Zaproszenia Discord | Plan | Blokowanie obcych invite i wyjątki partnerskie |
| Scam/phishing | Plan | Reputacja domen, heurystyki i szybka kwarantanna |
| Spam/flood | Plan | Wiadomości na okno czasu, burst i adaptacyjne progi |
| Duplikaty | Plan | Identyczne/podobne wiadomości i obrazy |
| Caps, znaki, emoji | Plan | Progi procentowe i długościowe |
| Mention spam | Plan | User/role/everyone/here z różnymi wagami |
| Attachment safety | Plan | Typ, rozmiar, rozszerzenie i opcjonalny skaner |
| Raid detection | Plan | Tempo joinów, wiek konta, zachowanie i wspólne cechy |
| Anti-nuke | Plan | Alarmy dla masowych banów, kanałów i ról; ograniczenia aktora |
| Verification | Plan | Button, challenge, pytania, role gate i kwarantanna |
| Account-age gate | Plan | Progi, wyjątki i kolejka ręcznej akceptacji |
| Alt risk scoring | Ograniczone | Tylko transparentne sygnały; bez fingerprintingu i ukrytego śledzenia |
| Native Discord AutoMod sync | Plan | Odczyt i zarządzanie regułami AutoMod z audytem |
| Actions | Plan | Delete, alert, warn, timeout, quarantine, slowmode, lockdown |
| Rule simulator | Plan | Test na przykładach przed publikacją |

## 4. Role, onboarding i dostęp

| Funkcja | Stan | Zakres |
|---|---|---|
| Autorole | Plan | Użytkownik/bot, opóźnienie, wymagania i wyjątki |
| Reaction roles | Plan | Legacy reaction menus |
| Button roles | Plan | Przyciski, exclusivity, required/forbidden roles |
| Select roles | Plan | Wiele opcji, limity, grupy i kategorie |
| Temporary roles | Plan | Automatyczne wygaśnięcie przez scheduler |
| Persistent roles | Plan | Przywracanie po ponownym wejściu z allowlistą |
| Role connections | Plan | Integracje z zewnętrznymi systemami, gdzie API pozwala |
| Welcome/leave | Plan | Wiadomości, embedy, obrazy i DM z limitami |
| Screening workflow | Plan | Reguły, pytania, akceptacja i przydział kanałów |
| Onboarding profiles | Plan | Wybór zainteresowań, gry, regionu i powiadomień |
| Membership lifecycle | Plan | Join, verification, active, inactive, alumni, banned |
| Inactive role | Plan | Okres braku aktywności, wyjątki i powiadomienie |
| Voice temp channels | Plan | Tworzenie, owner controls, limit, cleanup i transfer |

## 5. Tickety, support i workflow

| Funkcja | Stan | Zakres |
|---|---|---|
| Ticket panels | Plan | Button/select, wiele działów i branding |
| Formularze | Plan | Modal przed utworzeniem, wymagane pola i walidacja |
| Private channels/threads | Plan | Konfigurowalne modele uprawnień |
| Claim/unclaim | Plan | Przypisanie agenta i status |
| Transfer/escalation | Plan | Zespół, priorytet, poziom eskalacji |
| SLA | Plan | First response, resolution target i alerty |
| Tags/macros | Plan | Kategoryzacja i gotowe odpowiedzi |
| Participants | Plan | Add/remove użytkowników i obserwatorów |
| Transcript | Plan | HTML/JSON, załączniki, retencja i dostęp |
| Close/reopen/delete | Plan | Powód, opóźnienie i recovery window |
| Feedback | Plan | Ocena po zamknięciu i raport jakości |
| Ticket analytics | Plan | Wolumen, czas odpowiedzi, backlog, obciążenie zespołu |
| External helpdesk | Plan | Adaptery do systemów z oficjalnym API |

## 6. Muzyka, audio i voice

| Funkcja | Stan | Zakres |
|---|---|---|
| Songbird voice manager | Fundament | Rejestracja klienta voice i wymaganych intentów |
| Join/leave | Plan | Kanał użytkownika, kontrola stanu i timeout bezczynności |
| Play/search | Plan | Adaptery dostawców zgodne z ich regulaminami |
| Queue | Plan | Add/remove/move/clear/shuffle i pozycja |
| Pause/resume/stop/skip | Plan | Kontrola interakcyjna i komendy |
| Seek/volume | Plan | Limity głośności i walidacja |
| Loop | Plan | Track, queue, segment |
| Autoplay | Plan | Radio oparte na legalnym źródle/rekomendacji dostawcy |
| Playlisty/favorites | Plan | Serwerowe i użytkownika, import zgodny z API |
| DJ role | Plan | Uprawnienia, kanał muzyczny i bypass vote |
| Vote skip | Plan | Próg zależny od aktywnych słuchaczy |
| 24/7 | Plan | Kontrolowane utrzymanie połączenia i limity zasobów |
| Filtry DSP | Plan | Equalizer, bass, karaoke, speed/pitch z ochroną CPU |
| Radio/streams | Plan | HTTP streams i allowlisty |
| Metadata/lyrics | Ograniczone | Tylko przez licencjonowane lub dozwolone API |
| Recording | Ograniczone | Jawna zgoda, widoczne powiadomienie, retencja i prawo lokalne |
| Transcription | Ograniczone | Opt-in, redakcja danych, limity i provider abstraction |
| Multi-node audio | Plan | Voice nodes, health, failover i routing regionu |

## 7. Społeczność i rozrywka

| Funkcja | Stan | Zakres |
|---|---|---|
| `/meme` w Lua | Gotowe | Pierwszy przykładowy moduł fun |
| Meme feeds | Plan | Źródła z dozwolonym API i moderacja NSFW |
| Reaction images | Plan | Szablony, cooldown i content policy |
| Quotes | Plan | Dodawanie, usuwanie, wyszukiwanie i zgody |
| Starboard | Plan | Progi, self-star policy, kanały, załączniki i edycje |
| Counting | Plan | Kanały, zasady, rekordy i anti-cheat |
| Trivia | Plan | Kategorie, sesje, ranking i własne pytania |
| Minigry | Plan | Tic-tac-toe, connect four, word games, quizy |
| Polls | Plan | Natywne i rozszerzone, anonimowość opcjonalna |
| Suggestions | Plan | Statusy, głosy, komentarze, roadmapa i decyzja staffu |
| Confessions | Ograniczone | Moderacja, rate limit, abuse reporting i brak obietnicy pełnej anonimowości |
| Birthdays | Plan | Strefy czasowe, prywatność i role czasowe |
| Partnerships | Plan | Formularze, kolejka, cooldown i tracking |
| Bump reminders | Plan | Harmonogram i detekcja komend wspieranych botów |
| Server directory | Plan | Profile społeczności, wyszukiwanie i zasady publikacji |

## 8. Poziomy, reputacja i ekonomia

| Funkcja | Stan | Zakres |
|---|---|---|
| XP tekstowe | Plan | Cooldown, długość, jakość i anti-farm |
| XP voice | Plan | Aktywność, AFK, self-deaf, minimalna liczba osób |
| Poziomy 1–100+ | Plan | Krzywe, sezony i prestiż |
| Role rewards | Plan | Add/replace, stack i kontrola hierarchii |
| Rank card | Plan | Branding, dostępność i cache |
| Leaderboards | Plan | Serwerowe, sezonowe, role i kanały |
| Reputation | Plan | Cooldown, powód, historia i anti-abuse |
| Currency | Plan | Ledger zamiast tylko licznika salda |
| Daily/streak | Plan | Strefy czasowe i ochrona przed manipulacją |
| Shop/inventory | Plan | Role, przedmioty, stock i wygaśnięcie |
| Transfers | Plan | Limity, podatek opcjonalny, fraud signals |
| Games of chance | Ograniczone | Domyślnie wyłączone; zgodność z prawem i brak realnych pieniędzy |
| Admin adjustments | Plan | Powód, podwójne potwierdzenie dla dużych zmian i audyt |
| Economy export/reset | Plan | Snapshot, sezon i rollback |

## 9. Powiadomienia i integracje

| Funkcja | Stan | Zakres |
|---|---|---|
| RSS/Atom | Plan | Polling, deduplikacja, szablony i filtry |
| YouTube | Plan | Oficjalne API/webhooki, nowe filmy i live |
| Twitch | Plan | EventSub, online/offline, kategorie i role |
| GitHub | Plan | Push, releases, issues, PR, workflow status |
| Steam | Plan | News/status, gdzie oficjalne endpointy pozwalają |
| Game servers | Plan | Adaptery statusu, whitelisty i RCON tylko w secret brokerze |
| Reddit | Plan | Oficjalne API, filtry subreddit/flair |
| Social platforms | Ograniczone | Tylko stabilne, dozwolone API danego dostawcy |
| Generic webhooks | Plan | Podpisy, replay protection i schematy payload |
| Outgoing HTTP | Ograniczone | Nazwane integracje, allowlist, timeout i redakcja sekretów |
| Email | Plan | Transakcyjne powiadomienia panelu, nie masowy spam |
| Calendar | Plan | iCal/Google/Microsoft przez jawne połączenia |
| Status pages | Plan | Incident notifications i component mapping |

## 10. Harmonogramy, wydarzenia i giveaway

| Funkcja | Stan | Zakres |
|---|---|---|
| Schemat scheduled jobs | Fundament | Run time, retry, lock, completion i last error |
| Reminders | Plan | Jednorazowe i cykliczne, user/guild timezone |
| Cron automation | Plan | Walidacja, preview kolejnych uruchomień i limits |
| Giveaways | Plan | Role requirements, weighted entries opcjonalne, reroll i audit |
| Events/RSVP | Plan | Capacity, waitlist, reminders i attendance |
| Poll deadlines | Plan | Automatyczne zamknięcie i publikacja wyniku |
| Temporary resources | Plan | Role, channel, timeout, ban i cleanup |
| Scheduled messages | Plan | Draft, preview, timezone i approval workflow |
| Calendar view | Plan | Panel miesięczny/tygodniowy i filtry modułów |

## 11. Analityka, logi i obserwowalność

| Funkcja | Stan | Zakres |
|---|---|---|
| Structured tracing | Gotowe | Logi procesu API i bota bez sekretów |
| Health endpoint | Gotowe | Status, wersja i uptime API |
| Audit schema | Fundament | Actor, source, action, resource, before/after i request ID |
| Member analytics | Plan | Join/leave, retention, verification conversion |
| Activity analytics | Plan | Wiadomości, voice, aktywne dni i kanały |
| Moderation analytics | Plan | Cases, actions, repeat offenders i response time |
| Ticket analytics | Plan | SLA, backlog, resolution i satisfaction |
| Command analytics | Plan | Usage, latency, error rate i top commands |
| Music analytics | Plan | Playtime, providers, failures bez nadmiernego śledzenia |
| Lua telemetry | Plan | CPU proxy, instructions, memory, errors i disabled plugins |
| Metrics | Plan | Prometheus/OpenTelemetry, shard i worker metrics |
| Distributed tracing | Plan | Request/event correlation przez procesy |
| Live logs | Plan | RBAC, redakcja i ograniczony zakres |
| Export | Plan | CSV/JSON z kontrolą dostępu i retencją |
| Status page | Plan | Shards, voice nodes, queues, DB, Redis i integracje |

## 12. Panel WWW i control plane

| Funkcja | Stan | Zakres |
|---|---|---|
| Responsywny shell UI | Gotowe | Profesjonalny panel startowy bez zewnętrznego frameworka |
| Health/meta API | Gotowe | Dane bieżącego etapu i modułów |
| Discord OAuth2 | Plan | Authorization Code, state, cookies i bezpieczne sesje |
| Guild selector | Plan | Tylko serwery, którymi użytkownik może zarządzać |
| RBAC panelu | Plan | Owner/admin/mod/support/viewer i własne role |
| Module manager | Plan | Enable/disable, dependency checks i rollout |
| Formularze konfiguracji | Plan | Schematy, walidacja, defaults, preview i diff |
| Visual automation builder | Plan | Trigger, conditions, actions, branches, delays |
| Lua editor | Ograniczone | Lint, test sandbox, capabilities, review, publish i rollback |
| Secret manager | Plan | Szyfrowanie, redakcja, rotacja i nazwane użycie |
| Test/simulate | Plan | Fixture zdarzeń bez wykonywania prawdziwej akcji |
| Audit viewer | Plan | Filtry, diff i eksport |
| Analytics dashboards | Plan | Zakres zależny od roli i retencji |
| Import/export | Plan | Wersjonowany bundle bez sekretów |
| Backups/restore | Plan | Snapshot konfiguracji i test restore |
| Feature flags | Plan | Per guild, cohort, procent i kill switch |
| Multi-language UI | Plan | PL/EN/DE jako pierwsze locale |
| Accessibility | Plan | Keyboard, focus, contrast, reduced motion i screen readers |

## 13. Platforma deweloperska Lua

| Funkcja | Stan | Zakres |
|---|---|---|
| Plugin entrypoint | Gotowe | `plugins/<id>/main.lua` |
| Metadata | Gotowe | Name, version, description |
| Command handlers | Gotowe | Typed context i tekstowa odpowiedź |
| Memory limit | Gotowe | Limit ustawiany przez konfigurację |
| Instruction limit | Gotowe | VM hook przerywający runaway script |
| Dangerous globals removed | Gotowe | Brak OS, IO, package, debug i dynamic load |
| Duplicate validation | Gotowe | Plugin i command name uniqueness |
| Response validation | Gotowe | Pusta/za długa odpowiedź odrzucona |
| Typed command options | Plan | Schemat opcji i generowanie Discord builders |
| Event handlers | Plan | Message/member/reaction/voice/thread/event hooks |
| Capabilities | Plan | Manifest, approval i runtime authorization |
| Scoped storage | Plan | Per guild/plugin namespace i quotas |
| Scheduler API | Plan | Named jobs, payload schema i cancellation |
| HTTP broker | Ograniczone | Allowlisted connectors, redacted secrets i quotas |
| Discord action broker | Plan | Safe typed methods zamiast surowego HTTP |
| Hot reload | Plan | Validate, warm, atomic swap i rollback |
| Plugin migrations | Plan | Versioned, transactional i tested |
| CLI tooling | Plan | New, lint, test, pack, sign, publish |
| Local simulator | Plan | Fixtures, fake clock i deterministic IDs |
| Marketplace | Plan | Review, signatures, permissions, updates i trust levels |
| Per-tenant isolation | Plan | Separate runtime/pool and fair scheduling |

## 14. Opcjonalne funkcje AI

| Funkcja | Stan | Zakres |
|---|---|---|
| Provider abstraction | Plan | Własne klucze lub centralny provider z limitami |
| Server Q&A | Plan | Jawnie wybrane źródła i cytowanie |
| Thread summaries | Plan | Zakres kanałów, zgoda i redakcja danych |
| Ticket triage | Plan | Sugestia kategorii/priorytetu, decyzja człowieka |
| Moderation assistance | Ograniczone | Wyjaśnialne sygnały; brak autonomicznej ciężkiej kary bez reguł |
| Translation | Plan | Locale i kontrola retencji |
| Content drafting | Plan | Ogłoszenia, FAQ, event copy i mod macros |
| Semantic search | Plan | Indeks tylko zatwierdzonych treści |
| Quotas/cost controls | Plan | Per guild/user/model i twardy budżet |
| Safety filters | Plan | Provider + reguły produktu + audit |
| Data controls | Plan | Opt-in, retention, delete/export i no-training flags zależne od providera |

## 15. Operacje i infrastruktura

| Funkcja | Stan | Zakres |
|---|---|---|
| Rust workspace | Gotowe | Oddzielne app/crate boundaries |
| Docker images | Gotowe | Multi-stage, non-root runtime i Opus/FFmpeg |
| Docker Compose | Gotowe | API, bot, PostgreSQL i Redis |
| PostgreSQL migration | Fundament | Pierwsze tabele domenowe |
| Redis service | Fundament | Gotowy kontener; integracja w kodzie w następnym etapie |
| CI | Gotowe | Format, clippy, test, build, JS, Compose i API smoke |
| Graceful API shutdown | Gotowe | Ctrl+C i SIGTERM |
| Bot reconnect | Fundament | Zapewniany przez klienta Gateway |
| Rate limiting | Plan | Discord-aware i własne limity per actor/action |
| Queue workers | Plan | Retry, delay, priority, DLQ i idempotency |
| Object storage | Plan | Transcripts, exports i assets z retencją |
| Backups | Plan | DB, object storage, restore drills i RPO/RTO |
| Secrets management | Plan | KMS/Vault-compatible abstraction |
| Horizontal scaling | Plan | Shards, ownership routing, leases i stateless API |
| Multi-region | Plan | Dopiero po pomiarach; voice/data locality i failover |
| Deployment | Plan | Health/readiness, migrations, rolling/canary i rollback |
| SLO/alerts | Plan | Availability, latency, queue age, errors i Discord disconnects |

## 16. Funkcje właściciela platformy

| Funkcja | Stan | Zakres |
|---|---|---|
| `/ping` | Gotowe | Diagnostyka z Lua |
| `/about` | Gotowe | Informacja o produkcie |
| Status shardów | Plan | Latency, sessions, guild counts i reconnects |
| Plugin reload | Plan | Owner-only, walidacja i atomic swap |
| Maintenance mode | Plan | Per module/guild/global, komunikat i bypass owner |
| Graceful shutdown | Plan | Drain interactions/jobs i zamknięcie voice |
| Safe restart request | Plan | Przez orchestrator, nie `process::exit` z publicznej komendy |
| Feature kill switch | Plan | Natychmiastowe wyłączenie modułu lub integracji |
| Support impersonation | Ograniczone | Brak pełnego impersonation; tylko audytowany support view |
| Data export/delete | Plan | Obsługa żądań prywatności i polityk retencji |

## Funkcje świadomie wykluczone

ZuckerBot nie będzie implementował self-botów, używania tokenów kont użytkowników, masowego niezamówionego DM, raidów/nuke jako narzędzia ofensywnego, obchodzenia rate limitów, credential theft, stealth recording ani mechanizmów omijających zasady i zabezpieczenia dostawców treści.

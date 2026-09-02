# Analiza możliwości Discord App/Bot API

Stan analizy: **2 września 2026**. Źródłem prawdy są oficjalne dokumenty Discord Developer Platform. Funkcje zależne od zewnętrznych serwisów, takich jak dostawcy muzyki, YouTube, Twitch lub systemy AI, wymagają osobnej weryfikacji ich API i regulaminów.

## Co może obejmować nowoczesna aplikacja Discord

### Interakcje aplikacji

Discord obsługuje komendy typu chat input, user i message, a także entry point aplikacji. Komendy mogą mieć typowane opcje, subkomendy, lokalizacje, autocomplete, konteksty instalacji i wykonania oraz domyślne uprawnienia. Odpowiedzi mogą wykorzystywać wiadomości, embedy, pliki, przyciski, select menus, modale i kolejne wiadomości webhookowe.

**Wpływ na ZuckerBot:** obecny `CommandSpec` jest celowo minimalną wersją kontraktu. Następna wersja SDK musi objąć typowane opcje, komponenty, modale, autocomplete i context commands bez przekazywania Lua surowego klienta HTTP.

### Gateway, zdarzenia i sharding

Gateway dostarcza zdarzenia czasu rzeczywistego dotyczące serwerów, członków, wiadomości, reakcji, kanałów, wątków, voice state, scheduled events i innych zasobów. Część danych wymaga privileged intents. Duże boty muszą stosować sharding i respektować limity identyfikacji oraz wysyłania ramek.

**Wpływ na ZuckerBot:** privileged intents pozostają domyślnie wyłączone. Każdy moduł będzie deklarował wymagane zdarzenia, a router zbuduje minimalny zestaw intentów. Automatyczny sharding jest już używany przez klienta Serenity.

### Moderacja i Auto Moderation

Bot może wykonywać działania moderacyjne, o ile posiada odpowiednie uprawnienia i znajduje się wystarczająco wysoko w hierarchii ról. Discord udostępnia także zasoby Auto Moderation do zarządzania regułami, triggerami, wyjątkami i akcjami.

**Wpływ na ZuckerBot:** każda akcja musi przejść kontrolę uprawnień użytkownika, bota, hierarchii ról, capability pluginu, limitów i audytu. Operacje masowe potrzebują dry-run, twardego limitu i raportu częściowych błędów.

### Voice

Discord Voice jest osobnym protokołem obejmującym połączenie głosowe, negocjację szyfrowania, RTP i audio Opus. Zarządzanie muzyką wymaga state machine, kolejki, reconnectów, limitów zasobów oraz legalnego adaptera źródeł audio.

**Wpływ na ZuckerBot:** Songbird został zarejestrowany jako warstwa protokołu voice. Odtwarzacz, provider abstraction, kolejka, DJ permissions i failover są osobnym etapem, a nie kodem umieszczonym w handlerze komendy.

### OAuth2 i instalacja

OAuth2 umożliwia logowanie użytkownika do panelu i autoryzację aplikacji. Instalacja aplikacji może dotyczyć serwera lub użytkownika zależnie od konfiguracji i dostępnych kontekstów.

**Wpływ na ZuckerBot:** panel użyje Authorization Code Grant, ochrony `state`, bezpiecznej sesji i bieżącej kontroli uprawnień użytkownika na wybranym serwerze. Token bota nie trafia do panelu ani Lua.

### Activities, aplikacje osadzone i monetyzacja

Platforma wspiera aplikacje uruchamiane w kliencie Discord oraz zasoby związane z produktami, SKU i uprawnieniami premium. Są to odrębne obszary produktu i nie należy mieszać ich z podstawowym procesem bota.

**Wpływ na ZuckerBot:** Activities mogą w przyszłości dostarczyć interaktywne gry lub narzędzia serwerowe, a premium entitlements — kontrolę dostępu do płatnych funkcji. Oba obszary wymagają osobnych kontraktów, polityk i testów.

### Rate limits i niezawodność

Discord publikuje limity globalne, per-route i Gateway. Klient ma reagować na nagłówki rate limit, `retry_after` oraz błędy, zamiast kodować na sztywno liczby. Niektóre zdarzenia mogą zostać ponowione lub nadejść po reconnect.

**Wpływ na ZuckerBot:** adapter Discorda odpowiada za retry i limity API. Workery muszą używać idempotency keys, backoffu z jitterem, kolejki błędów i deduplikacji. Lua nie może wykonywać surowych żądań do Discord API.

## Obszary uwzględnione w macierzy produktu

Pełna macierz w `FEATURE_MATRIX.md` obejmuje:

- komendy, komponenty, modale, konteksty, wątki, fora, ankiety, wydarzenia i webhooki;
- moderację, AutoMod, anti-spam, anti-raid, verification, appeals i modmail;
- role, onboarding, temporary voice, tickety i workflow engine;
- muzykę, voice, kolejki, playlisty, DSP i kontrolowane nagrywanie/transkrypcję;
- XP, reputację, ekonomię ledgerową, rankingi, wydarzenia i giveaway;
- integracje, harmonogramy, analitykę, audyt i obserwowalność;
- panel WWW, OAuth2, RBAC, edytor Lua, sekrety, import/export i backup;
- bezpieczny ekosystem pluginów Lua oraz opcjonalne moduły AI.

## Granice, których aplikacja nie może omijać

- Bot nie może działać jako self-bot na tokenie użytkownika.
- Uprawnienia Discorda i hierarchia ról są nadrzędne wobec konfiguracji ZuckerBot.
- Rate limitów, systemów antynadużyciowych, DRM ani ograniczeń zewnętrznych usług nie wolno obchodzić.
- Nagrywanie i transkrypcja głosu wymagają jawnego włączenia, widocznej informacji i zgodności prawnej.
- „Wszystko” oznacza szeroką, wersjonowaną platformę w granicach oficjalnego API, bezpieczeństwa i prawa, a nie nieograniczony dostęp do kont użytkowników lub cudzych usług.

## Oficjalne źródła

- Application Commands: https://discord.com/developers/docs/interactions/application-commands
- Receiving and Responding to Interactions: https://discord.com/developers/docs/interactions/receiving-and-responding
- Message Components: https://discord.com/developers/docs/interactions/message-components
- Gateway: https://discord.com/developers/docs/events/gateway
- Gateway Intents: https://discord.com/developers/docs/events/gateway#gateway-intents
- Voice Connections: https://discord.com/developers/docs/topics/voice-connections
- OAuth2: https://discord.com/developers/docs/topics/oauth2
- Auto Moderation: https://discord.com/developers/docs/resources/auto-moderation
- Rate Limits: https://discord.com/developers/docs/topics/rate-limits
- Discord Developer changelog: https://discord.com/developers/docs/change-log
- Embedded App SDK: https://discord.com/developers/docs/developer-tools/embedded-app-sdk
- Monetization: https://discord.com/developers/docs/monetization/overview

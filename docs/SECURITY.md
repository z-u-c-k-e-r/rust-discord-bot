# Model bezpieczeństwa

## Zasady

1. Najmniejsze wymagane uprawnienia.
2. Brak sekretów w Lua, logach, odpowiedziach API i frontendzie.
3. Jawne capabilities dla pluginów.
4. Pełny audyt zmian konfiguracji i operacji administracyjnych.
5. Limity czasu, pamięci, instrukcji, rozmiaru i częstotliwości.
6. Bezpieczne wartości domyślne oraz wyłączone privileged intents.
7. Możliwość cofnięcia lub zatrzymania automatyzacji.
8. Rozdzielenie uprawnień właściciela, administratora, moderatora, supportu i autora pluginu.

## Token Discorda

Token jest pobierany tylko ze środowiska procesu bota. Nie jest serializowany, logowany, przekazywany do Lua ani API panelu. Wyciek wymaga natychmiastowej rotacji w Discord Developer Portal.

## Discord OAuth2

Panel docelowo użyje Authorization Code Grant z `state`, PKCE tam, gdzie ma zastosowanie, krótkimi sesjami, rotacją cookies, `HttpOnly`, `Secure`, `SameSite`, ochroną CSRF oraz ścisłą listą redirect URI. Użytkownik zobaczy tylko serwery, którymi może zarządzać. Każda konfiguracja będzie dodatkowo sprawdzała bieżące uprawnienia na Discordzie.

## Sandbox Lua

Aktualne zabezpieczenia:

- bezpieczny konstruktor Lua;
- usunięte biblioteki systemowe i dynamiczne ładowanie kodu;
- limit pamięci;
- hook ograniczający liczbę instrukcji;
- minimalny kontekst bez sekretów;
- walidacja kontraktu wejścia/wyjścia;
- seryjne wykonanie przez mutex jednego runtime.

Kolejne wymagania przed włączeniem edytora WWW i niezaufanych pluginów:

- osobny runtime per tenant albo izolowana pula runtime;
- timeout ścienny poza limitem instrukcji;
- capability broker zamiast bezpośrednich funkcji Rust;
- limity per plugin, serwer i użytkownik;
- podpisy oraz checksumy wersji;
- skan statyczny i test przed publikacją;
- wersjonowanie, rollback i automatyczny kill switch;
- ochrona przed exfiltracją danych przez dozwolone endpointy.

## Moderacja i operacje masowe

Ban, kick, timeout, role, kanały, purge i lockdown wymagają:

- uprawnienia użytkownika i bota;
- kontroli hierarchii ról;
- jawnego powodu;
- limitu liczby celów;
- potwierdzenia lub dry-run dla operacji masowych;
- unikalnego numeru sprawy;
- wpisu audytowego;
- bezpiecznego raportu częściowych niepowodzeń.

## Muzyka i nagrywanie

Warstwa dostawców audio będzie respektowała ich regulaminy, licencje oraz prawa autorskie. Projekt nie będzie zawierał mechanizmów obchodzących DRM, płatne dostępy lub techniczne ograniczenia usług. Nagrywanie, transkrypcja i analiza głosu mogą działać tylko po jawnym włączeniu, widocznym powiadomieniu uczestników i zgodnie z prawem właściwej jurysdykcji.

## Nadużycia wykluczone z projektu

- self-boty i używanie tokenów użytkowników;
- token grabbery i credential stuffing;
- raidy, nukowanie serwerów, masowe DM i spam;
- obchodzenie rate limitów lub systemów antynadużyciowych Discorda;
- ukryte śledzenie użytkowników;
- nieautoryzowane nagrywanie;
- automatyczne działania omijające hierarchię ról i zasady serwera.

## Zgłaszanie podatności

Nie publikuj exploita w publicznym issue. Użyj prywatnego GitHub Security Advisory. Zgłoszenie powinno zawierać wpływ, minimalny sposób odtworzenia, dotkniętą wersję i propozycję ograniczenia ryzyka. Nie umieszczaj prawdziwych tokenów ani danych użytkowników.

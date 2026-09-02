#!/usr/bin/env python3
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]

compose = root / "docker-compose.yml"
text = compose.read_text(encoding="utf-8")
text = text.replace("      - ./migrations:/docker-entrypoint-initdb.d:ro\n", "")
compose.write_text(text, encoding="utf-8")

status = root / "STATUS.md"
text = status.read_text(encoding="utf-8")
text = text.replace(
    "`platform-foundation` — pierwszy uruchamialny przekrój Rust + Lua + Discord + WWW.",
    "`control-plane-storage` — trwała infrastruktura PostgreSQL/Redis rozwijana jako stacked PR nad zweryfikowanym fundamentem.",
)
section = """

## Bieżąca gałąź `feat/control-plane-storage`

W tej gałęzi dostarczono:

- crate `zuckerbot-storage` z pulą PostgreSQL i klientem Redis;
- osadzone migracje SQLx uruchamiane przez jawną flagę;
- osobne liveness `/health` i readiness `/ready`;
- repository contracts dla serwerów i konfiguracji modułów;
- optymistyczną kontrolę współbieżności przez `expected_version`;
- walidację identyfikatorów i limit konfiguracji 64 KiB;
- redakcję tokenu Discorda oraz adresów połączeń z `Debug`;
- prawdziwe testy integracyjne PostgreSQL/Redis w GitHub Actions.

Publiczne endpointy zapisu konfiguracji pozostają zablokowane do wdrożenia Discord OAuth2, sesji, CSRF i RBAC.
"""
if "## Bieżąca gałąź `feat/control-plane-storage`" not in text:
    text += section
status.write_text(text, encoding="utf-8")

worklog = root / "WORKLOG.md"
text = worklog.read_text(encoding="utf-8")
entry = """

## 2026-09-02 — trwała infrastruktura control plane

### Problem

Fundament miał schemat SQL i kontenery, ale proces API nie korzystał z PostgreSQL ani Redis. `/health` nie odróżniał żywego procesu od aplikacji gotowej do obsługi konfiguracji. Brakowało także ochrony przed nadpisaniem równoczesnych zmian administratorów.

### Decyzje

- liveness i readiness są osobnymi endpointami;
- połączenia są tworzone leniwie, aby API mogło raportować `503` zamiast wpadać w pętlę restartów;
- migracje są osadzone w binarce, lecz ich wykonanie kontroluje jawna flaga;
- Docker nie uruchamia równolegle surowych skryptów init i migratora SQLx;
- konfiguracja modułu używa optymistycznej wersji zamiast last-write-wins;
- publiczny zapis HTTP nie powstaje przed OAuth2, CSRF i RBAC;
- prawdziwe PostgreSQL i Redis są częścią CI;
- sekrety mają ręcznie redagowane implementacje `Debug`.

### Zmiany

- dodano crate `zuckerbot-storage`;
- dodano repository contracts i adapter PostgreSQL;
- dodano migrację wersji, wyniku audytu i sesji WWW;
- dodano równoległe readiness checks z timeoutami;
- rozszerzono API i workflow o testy prawdziwych zależności;
- usunięto podwójne wykonywanie migracji z inicjalizacji kontenera PostgreSQL.

### Walidacja

Lokalne formatowanie, kontrola typów, testy jednostkowe i kontrola JavaScript są częścią commita generującego. Pełny exact-head CI z PostgreSQL i Redis jest wymagany przed closeoutem stacked PR.
"""
if "## 2026-09-02 — trwała infrastruktura control plane" not in text:
    text += entry
worklog.write_text(text, encoding="utf-8")

for probe in ["TEST_LOCAL_UPLOAD", "CONNECTOR_PROBE"]:
    path = root / probe
    if path.exists():
        path.unlink()

subprocess.run(["cargo", "fmt", "--all"], cwd=root, check=True)
subprocess.run(["cargo", "check", "-p", "zuckerbot-storage", "-p", "zuckerbot-api"], cwd=root, check=True)
subprocess.run(["cargo", "test", "-p", "zuckerbot-storage", "--lib"], cwd=root, check=True)
subprocess.run(["node", "--check", "apps/api/static/app.js"], cwd=root, check=True)

lockfile = root / "Cargo.lock"
if lockfile.exists():
    lockfile.unlink()

for transient in [
    ".github/workflows/control-plane-finalize.yml",
    "tools/finalize-control-plane.py",
]:
    path = root / transient
    if path.exists():
        path.unlink()

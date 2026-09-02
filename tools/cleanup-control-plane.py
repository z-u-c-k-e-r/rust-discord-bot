#!/usr/bin/env python3
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]

compose = root / "docker-compose.yml"
text = compose.read_text(encoding="utf-8")
text = text.replace("      - ./migrations:/docker-entrypoint-initdb.d:ro\n", "")
compose.write_text(text, encoding="utf-8")

for transient in [
    "TEST_LOCAL_UPLOAD",
    "CONNECTOR_PROBE",
    ".github/workflows/control-plane-bootstrap.yml",
    ".github/workflows/control-plane-finalize.yml",
    ".github/workflows/control-plane-cleanup.yml",
    "tools/bootstrap-control-plane.py",
    "tools/finalize-control-plane.py",
    "tools/cleanup-control-plane.py",
]:
    path = root / transient
    if path.exists():
        path.unlink()

subprocess.run(["cargo", "fmt", "--all"], cwd=root, check=True)
subprocess.run(["cargo", "check", "-p", "zuckerbot-storage", "-p", "zuckerbot-api"], cwd=root, check=True)
subprocess.run(["cargo", "test", "-p", "zuckerbot-storage", "--lib"], cwd=root, check=True)
subprocess.run(["node", "--check", "apps/api/static/app.js"], cwd=root, check=True)

lockfile = root / "Cargo.lock"
if lockfile.exists():
    lockfile.unlink()

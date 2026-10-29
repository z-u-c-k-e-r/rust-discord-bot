# Security design

## Trust boundaries

The deployment has four distinct trust levels:

1. infrastructure secrets and the Rust process
2. authenticated dashboard administrators
3. reviewed Lua modules
4. Discord users and event payloads

Lua modules are not infrastructure code. Even first-party modules run through the same constrained action boundary.

## Secret handling

Secrets are read from environment variables:

- `DISCORD_TOKEN`
- `DISCORD_CLIENT_SECRET`
- `DATABASE_URL`

They must never be committed, rendered in the dashboard, passed to Lua or included in structured logs. Production deployments should use a secret manager or container secret facility rather than a shared `.env` file.

## Lua isolation

Current controls:

- fresh VM for every invocation
- vendored Lua 5.4
- memory ceiling
- global instruction-count hook covering the main thread and every new Lua thread
- disabled filesystem, operating-system, module-loader and debug libraries
- disabled dynamic loading (`load`, `loadfile`, `dofile`)
- disabled coroutines and direct garbage-collector controls
- disabled `pcall` and `xpcall`, preventing scripts from swallowing the instruction-limit error
- no native network API
- no direct database API
- no Discord client objects
- typed context serialization
- typed action deserialization
- maximum 25 actions per invocation
- validation before action execution

These controls reduce risk from accidental loops, large allocations and direct capability abuse. They do not make arbitrary third-party Lua automatically trustworthy. Module review remains required.

Because error-catching primitives are intentionally unavailable, module code should validate inputs and return safe fallback actions instead of using exception-driven control flow. Recoverable host operations are executed only after the Lua handler returns and are reported by the Rust action executor.

Future hardening candidates:

- process-level sandbox workers
- seccomp/AppArmor profile
- signed module packages
- per-module capability declarations
- module provenance and hash audit
- fuzzing of manifest and action decoding
- bounded VM pools with state reset proofs

## Discord authorization

Privileged command actions perform two checks:

1. invoking member permissions
2. bot/application permissions supplied by the interaction

Moderation actions additionally fetch current guild/member information and verify:

- the target is not the guild owner
- the bot's highest role is above the target
- the invoking moderator's highest role is above the target, unless the moderator is the guild owner
- a moderator cannot target themselves

Role actions verify that the managed role is below the bot and invoking moderator.

Discord remains the final authority and can reject an operation because of changed permissions, hierarchy or rate limits.

## Dashboard security

Implemented:

- Discord authorization-code OAuth2 flow
- one-time random state values
- ten-minute OAuth2 state lifetime
- Discord OAuth2 access token retained in server session memory only for live permission revalidation (never serialized to client)
- opaque HttpOnly session cookie
- SameSite=Lax
- optional Secure flag
- session expiry
- live guild authorization revalidated against Discord with short-lived caching (60s TTL) for sensitive/write routes (fail-closed)
- CSRF token required for writes
- 64 KiB configuration limit
- audit event for configuration changes

Production requirements:

- terminate HTTPS before the application
- set `SESSION_COOKIE_SECURE=true`
- restrict dashboard origin and reverse-proxy headers
- configure request-rate limits at the proxy
- use a persistent/distributed session store before horizontal scaling
- rotate Discord credentials after suspected exposure
- retain audit logs according to a documented privacy policy

## Media source security and egress network hardening

Music input invokes external media resolution and streaming via `yt-dlp` / Songbird. Protection against Server-Side Request Forgery (SSRF) combines three defense-in-depth layers:

### 1. Application-level input validation
- User-provided URLs must use HTTPS (`https://`).
- The initial hostname is strictly validated against `MUSIC_ALLOWED_HOSTS` (`youtube.com`, `soundcloud.com`, `bandcamp.com`, etc.). Subdomains must terminate with a registered root domain (`host == candidate || host.ends_with(".{candidate}")`).
- IP literals in URLs are checked against private, loopback, link-local, and reserved ranges (`127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `::1`, `fc00::/7`, `fe80::/10`) and rejected immediately.
- Plain text is treated strictly as a search query, never as a raw network endpoint.
- Lua modules cannot modify or bypass the host allowlist.

### 2. HTTP client egress redirect policy
- The media HTTP client enforces a strict redirect policy:
  - Follow-up redirects must remain on HTTPS (`https://`).
  - Redirect targets to private, loopback, carrier-grade NAT (`100.64.0.0/10`), link-local, or cloud metadata endpoints (`169.254.169.254`) are intercepted and rejected.
  - Redirect chains are bounded to at most 10 hops.

### 3. Container and network-level isolation
Production deployments must isolate media workers at the host/container firewall layer:
- Run media containers in an isolated Docker network bridge without host network access.
- Block egress to cloud metadata services (`169.254.169.254/32`).
- Block outbound traffic from media workers to internal VPC ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`) and local daemon ports (PostgreSQL `5432`, Dashboard `8080`).
- Ensure DNS resolvers drop or filter responses resolving to RFC 1918 addresses (DNS rebinding prevention).

## Message safety

Bot-generated messages use an empty Discord allowed-mentions policy. User-provided text should also pass through `zuckerbot.escape_mentions`.

Future rich-message actions must define explicit mention, attachment and embed limits rather than inheriting broad Discord defaults.

## Database

- application queries use SQLx parameter binding
- module configuration is stored as JSONB
- configuration payloads are size-limited
- audit events are append-only through the current application API
- database users should receive only the privileges needed by migrations and runtime

Production should separate migration credentials from runtime credentials.

## Logging and privacy

Do not log:

- bot or OAuth2 tokens
- database passwords
- complete private message contents
- unnecessary personal data
- raw authorization headers or cookies

New event logs require retention and redaction decisions. Voice recording is explicitly outside the accepted scope until consent, jurisdiction and deletion behavior are designed.

## Dependency and release security

Before production release:

- pin and commit `Cargo.lock`
- run `cargo audit` or an equivalent advisory scanner
- generate an SBOM
- scan container images
- sign release images
- enable protected branches and required CI
- run dependency updates through pull requests
- maintain a rollback image

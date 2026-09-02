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
- instruction-count hook
- disabled filesystem, operating-system, module-loader and debug libraries
- no native network API
- no direct database API
- no Discord client objects
- typed context serialization
- typed action deserialization
- maximum 25 actions per invocation
- validation before action execution

These controls reduce risk from accidental loops, large allocations and direct capability abuse. They do not make arbitrary third-party Lua automatically trustworthy. Module review remains required.

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
- Discord access token discarded after profile and guild lookup
- opaque HttpOnly session cookie
- SameSite=Lax
- optional Secure flag
- session expiry
- per-request manageable-guild authorization
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

## Media source security

Music input can invoke an external media resolver. To reduce SSRF exposure:

- URLs must use HTTPS
- URL hosts must match `MUSIC_ALLOWED_HOSTS`
- plain text is treated as a search query
- Lua cannot change the allowlist
- the runtime does not expose arbitrary HTTP actions

Production media workers should run in a separate network namespace with no access to metadata endpoints, databases or internal control-plane services.

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

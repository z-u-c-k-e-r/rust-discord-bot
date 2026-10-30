# Persistent moderation cases

ZuckerBot stores moderation history as versioned cases instead of treating a Discord kick, timeout or ban as an unstructured command response. This document covers the persistence contract introduced by the moderation case-management milestone.

## Data model

Every case has two identifiers:

- an internal UUID used by foreign keys;
- a monotonically increasing, human-readable case number scoped to one Discord guild.

A case records the guild, subject, moderator, action kind, status, severity, points, reason, source module, subject visibility, optional expiry, timestamps and optimistic-lock version. Supported action kinds are `warning`, `staff_note`, `timeout`, `kick`, `ban`, `unban`, `softban`, `automod` and `other`.

Statuses are:

- `active` — the record currently contributes to active moderation state;
- `expired` — its configured expiry has passed;
- `voided` — a moderator invalidated the record while preserving history.

Voiding never deletes a case. The actor, reason and timestamp are retained. Restoring a voided case creates another immutable event and chooses `active` or `expired` according to its expiry.

## Related records

A case owns three append-only collections:

- notes, with an independent `visible_to_subject` flag;
- evidence references, each with a short label and bounded value;
- events describing creation, edits, expiry, notes, evidence, voiding and restoration.

Deleting a case is deliberately not part of the service contract. Database cascade rules exist only for controlled retention or administrative teardown, not ordinary bot commands.

## Concurrency

Mutable case rows use an integer `version`. Edit, void and restore operations require the caller's expected version. A stale request returns `VersionConflict` rather than silently overwriting another moderator's change.

Per-guild case numbers are allocated through one atomic PostgreSQL upsert. Concurrent moderators therefore cannot receive the same case number.

## Expiration

Expired records are transitioned lazily before reads and statistics. PostgreSQL performs the transition in a transaction and appends an `expired` event for every changed record. The memory backend mirrors this behavior for deterministic tests.

This model avoids a correctness dependency on a scheduler while still allowing a future worker to run the same transition proactively.

## Validation limits

The Rust boundary validates input before storage:

| Field | Limit |
| --- | ---: |
| Reason / void reason | 2,000 characters |
| Note | 2,000 characters |
| Evidence label | 200 characters |
| Evidence value | 4,096 characters |
| Source module | 64 characters |
| Points | 0–10,000 |
| List result size | 1–100 |
| Future expiry | at most 3,653 days |

Discord snowflakes are stored as decimal text to avoid signed 64-bit overflow and are parsed back to `u64` at the repository boundary.

## Backends

`ModerationCaseStore` exposes the same API over:

- `MemoryModerationCaseStore` for tests and volatile development;
- `PostgresModerationCaseStore` for durable deployments.

The PostgreSQL schema is added by `migrations/0005_moderation_cases.sql` and updated by `migrations/0007_moderation_softban.sql`. Migration validation must run against the repository's current PostgreSQL image before merge.

## Security boundary

The durable case storage is decoupled from Discord/Lua authorization. The persistence layer accepts authorized actor and subject identifiers. The command and Lua execution layer in Rust enforces:

- guild context;
- moderator permissions (`MODERATE_MEMBERS`, `KICK_MEMBERS`, `BAN_MEMBERS`, `MANAGE_MESSAGES`, `ADMINISTRATOR`) and configured staff roles;
- target role hierarchy;
- subject-only filtering for self-service views (non-staff callers can only view cases where they are the subject and `visible_to_subject` is true, without internal staff notes);
- capability grants and strict schema validation for every Lua action;
- audit records in the database audit log for every case creation, note, void, and restore operation.

Lua submits typed case operations (`LuaAction::ModerationCase`). Lua does not receive SQL access, database credentials or an unrestricted repository handle.

## Web Dashboard & API

The control plane exposes moderation case management to authenticated guild moderators:

- `GET /api/guilds/{guild_id}/cases` — query cases with status/subject/pagination filters;
- `GET /api/guilds/{guild_id}/cases/{case_number}` — fetch case details, notes, evidence, and history;
- `POST /api/guilds/{guild_id}/cases/{case_number}/notes` — append moderator or subject-visible notes;
- `POST /api/guilds/{guild_id}/cases/{case_number}/void` — void case with required reason and version concurrency guard;
- `PATCH /api/guilds/{guild_id}/cases/{case_number}` — update case reason, severity, points, and visibility;
- `GET /api/guilds/{guild_id}/audit` — view searchable security and moderation audit trails;
- Interactive web dashboard UI under the `Moderacja` and `Dziennik audytu` tabs with responsive cards, status badges, details drawer, inline note entry, void dialogs, and reason editing.

## Required verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
```

CI must additionally apply every migration to PostgreSQL 18.6 in order and build the production container.

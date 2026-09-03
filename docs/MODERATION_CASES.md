# Persistent moderation cases

ZuckerBot stores moderation history as versioned cases instead of treating a Discord kick, timeout or ban as an unstructured command response. This document covers the persistence contract introduced by the moderation case-management milestone.

## Data model

Every case has two identifiers:

- an internal UUID used by foreign keys;
- a monotonically increasing, human-readable case number scoped to one Discord guild.

A case records the guild, subject, moderator, action kind, status, severity, points, reason, source module, subject visibility, optional expiry, timestamps and optimistic-lock version. Supported action kinds are `warning`, `staff_note`, `timeout`, `kick`, `ban`, `unban`, `automod` and `other`.

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

The PostgreSQL schema is added by `migrations/0005_moderation_cases.sql`. Migration validation must run against the repository's current PostgreSQL image before merge.

## Security boundary

This milestone deliberately separates durable case storage from Discord/Lua authorization. The persistence layer accepts already-authorized actor and subject identifiers but never performs Discord actions. The subsequent command integration must enforce, in Rust:

- guild context;
- moderator permissions and configured staff roles;
- target role hierarchy;
- subject-only filtering for self-service views;
- capability grants for every Lua module;
- audit records for every privileged operation.

Lua will only submit typed case operations. It will not receive SQL access, database credentials or an unrestricted repository handle.

## Required verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
```

CI must additionally apply every migration to PostgreSQL 18.6 in order and build the production container.

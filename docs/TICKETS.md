# Tickets and support workflows

The ticket system is a first-party Lua-controlled feature backed by a Rust authorization, Discord and persistence boundary. Lua defines the `/ticket` command tree and converts configuration into typed operations. Rust validates every operation, resolves live Discord state and performs privileged channel changes.

## Command surface

| Command | Purpose |
| --- | --- |
| `/ticket open` | Reserve a ticket and create a private text channel. |
| `/ticket list` | List the caller's active tickets or the staff queue. |
| `/ticket info` | Show state, priority, owner, claimant, participants and recent events. |
| `/ticket claim` | Assign an open ticket to a support agent. |
| `/ticket unclaim` | Return an assigned ticket to the queue. |
| `/ticket close` | Save a transcript, make members read-only and archive the channel. |
| `/ticket reopen` | Restore member access and move a closed ticket back to the open category. |
| `/ticket add` | Add a server member to the private ticket. |
| `/ticket remove` | Remove a non-owner participant. |
| `/ticket rename` | Rename the ticket channel using a sanitized Discord-safe slug. |
| `/ticket priority` | Set low, normal, high or urgent priority. |
| `/ticket transcript` | Persist and attach a bounded plaintext transcript. |

## Required configuration

`open_category_id` must identify a Discord category in the same guild. The remaining settings are optional:

- `archive_category_id` moves closed tickets into an archive category;
- `log_channel_id` receives lifecycle notices and transcript attachments;
- `support_role_ids` grants configured support roles access to newly created channels;
- `allowed_queues` and `default_queue` define accepted queues;
- `max_open_per_user` bounds active tickets per member;
- creator permissions separately control closing, renaming and participant management;
- transcript generation and the maximum captured message count are configurable.

IDs are represented as strings so Discord snowflakes do not lose precision in JSON or JavaScript.

## Privacy and authorization

New ticket channels receive explicit permission overwrites:

- `@everyone` is denied `View Channel`;
- the creator can view, write, attach files and read history;
- configured support roles can view and moderate messages;
- the bot receives only the channel capabilities required to manage the workflow.

The special `@everyone` role cannot be configured as a support role. Every category, log channel, role and member is resolved from the live Discord guild before use. Cross-guild channels are rejected. Allowed mentions are disabled by default and limited to one explicitly selected user for system notices.

Lua never receives the Discord token, PostgreSQL credentials or a raw Discord client. Only the trusted module id `tickets` may return ticket operations, and passive event handlers cannot invoke them.

## Persistence model

Production deployments use PostgreSQL tables for:

- ticket state and optimistic version numbers;
- participants;
- complete lifecycle events;
- plaintext transcripts.

Local development without `DATABASE_URL` uses an in-memory implementation with the same domain contract. Its contents disappear on restart.

Ticket creation uses a two-phase workflow:

1. Rust takes a per-guild/per-user advisory transaction lock, expires abandoned reservations, checks the active-ticket limit and stores a `provisioning` reservation.
2. Rust creates the private Discord channel and then activates the reservation with its channel id.

A failed Discord channel creation marks the reservation `failed`, preventing an abandoned reservation from permanently consuming quota. Reservations left in `provisioning` for more than ten minutes are expired on the next reservation attempt.

## Lifecycle and concurrency

The state machine is:

```text
provisioning -> open -> claimed -> closed
                    ^      |          |
                    +------+----------+
provisioning -> failed
```

PostgreSQL mutations lock the selected ticket row with `SELECT ... FOR UPDATE`. Claiming therefore has one winner even when multiple agents act concurrently. Every successful domain mutation and its ticket-history event are committed in the same database transaction.

Discord and PostgreSQL cannot participate in one distributed transaction. Channel changes use compensating actions where practical, and all failures are logged. The design prioritizes preserving a private channel and its audit evidence over destructive automatic cleanup.

## Transcripts

Transcripts:

- fetch message history in pages of at most 100 messages;
- are bounded by the configured message count and a safe attachment-size ceiling;
- include timestamps, message ids, author ids, text and attachment links;
- are stored in PostgreSQL before being sent as a Discord attachment;
- can be copied to the configured log channel;
- do not include bot tokens, dashboard sessions or other host secrets.

Plaintext was selected for the first version because it is portable, inspectable and safe to render without executing HTML or JavaScript. Retention periods, deletion requests and encryption-at-rest requirements must be defined by the operator before production use.

## Audit events

The ticket-specific event stream records reservation, opening, provisioning failure, claim, unclaim, participant changes, priority, rename, transcript, close and reopen operations. The platform-wide audit store receives the important privileged transitions for centralized administration and future dashboard analytics.

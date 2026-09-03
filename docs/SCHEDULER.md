# Persistent scheduler and reminders

The scheduler is a first-party Lua-controlled feature with a Rust execution and persistence boundary. Lua declares user-facing commands and returns validated `scheduler` actions. Rust authorizes, persists, leases and delivers every job.

## Commands

| Command | Audience | Purpose |
| --- | --- | --- |
| `/remind` | Member | Create a one-time reminder in the current channel. |
| `/reminders` | Member | List the caller's pending reminders. |
| `/remindcontrol` | Member | Cancel, pause or resume a caller-owned job. |
| `/schedulemessage` | Manage Server | Create a one-time or recurring server message in a selected channel. |
| `/schedules` | Manage Server | List pending jobs for the guild. |
| `/schedulecontrol` | Manage Server | Cancel, pause or resume any guild job. |

Accepted time forms include relative values such as `15m`, `1d 2h 30m`, Unix timestamps and RFC 3339 timestamps. Repeat intervals support `s`, `m`, `h`, `d` and `w`, are bounded to one year and cannot be shorter than 60 seconds.

## Authorization and isolation

- Scheduler actions are accepted only from the trusted module with id `scheduler`.
- Actions can be initiated only from a user command, not a passive event handler.
- Recurrence, cross-channel scheduling and guild-wide listing require Manage Server or Administrator.
- A custom target channel is resolved through Discord and must belong to the invoking guild.
- Members can mutate only jobs they created; authorized staff can mutate any job in the same guild.
- Job and channel identifiers are validated before persistence or Discord calls.
- Scheduled content never enables arbitrary user, role, `@everyone` or `@here` mentions. A job may explicitly notify only its creator.

## Persistence

Production jobs are stored in PostgreSQL. The in-memory implementation exists for local development and tests and is intentionally volatile.

The PostgreSQL implementation uses:

- a per-guild/per-creator advisory transaction lock while enforcing pending-job limits;
- `FOR UPDATE SKIP LOCKED` when workers claim due jobs;
- a worker id and lease timestamp;
- lease recovery for workers that terminate during delivery;
- bounded attempts and exponential retry delay;
- terminal `completed`, `cancelled` and `failed` states;
- transactional state transitions for create, control and delivery completion.

## Delivery semantics

Delivery is **at least once**. A process can send a Discord message and terminate before recording success, in which case another worker may retry after the lease expires. Exactly-once delivery cannot be guaranteed because Discord message creation and the local database update are separate systems without a shared transaction.

The design prioritizes not silently losing reminders. Future interaction components can expose duplicate-delivery diagnostics and manual replay controls without changing this persistence contract.

## Recurrence

`remaining_runs` counts executions including the next due execution. A finite recurring job decrements the value after each successful delivery and completes at zero. A null value represents an unlimited recurrence. The next run is based on the previous scheduled time, but is moved at least one second beyond completion if the worker was delayed, preventing a tight catch-up loop.

## Audit events

The existing audit store receives:

- `scheduled_job_created`;
- `scheduled_job_cancelled`;
- `scheduled_job_paused`;
- `scheduled_job_resumed`;
- `scheduled_job_delivered`;
- `scheduled_job_retry_scheduled`;
- `scheduled_job_failed`.

Audit failures are logged separately and do not turn a successfully delivered Discord message into a delivery retry.

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

## Delivery semantics, concurrency and lease management

Delivery is **at least once**. Exactly-once delivery across distributed network boundaries cannot be guaranteed because Discord message creation and the local database update are separate distributed systems without a shared 2PC/distributed transaction.

### Worker concurrency and batch claiming

- The scheduler worker operates with bounded concurrency managed via an asynchronous semaphore (configurable via the `SCHEDULER_CONCURRENCY` environment variable, defaulting to `4`).
- Rather than claiming large static batches that sit queued in memory while their leases tick away, workers claim only as many jobs as there are currently available concurrency permits (up to a batch ceiling of `10`).
- Claimed jobs begin execution immediately upon claiming.

### Active lease heartbeat (`renew_lease`)

- Jobs are claimed with a default lease window of `60 seconds` (`LEASE_SECONDS`).
- To prevent a slow delivery (due to Discord API latency, network congestion, or Discord rate limits) from expiring its lease and being erroneously reclaimed by another worker, active jobs run a background lease renewal heartbeat every `20 seconds` (`LEASE_RENEWAL_INTERVAL_SECONDS`).
- The heartbeat calls `renew_lease()`, updating `locked_at` and `updated_at` to the current timestamp.
- As long as the delivery task is alive, other workers cannot reclaim the job.
- When the job delivery completes or errors, the heartbeat task is aborted and the lease is released via `mark_succeeded()` or `mark_failed()`.

### Worker crash recovery and the crash window

- **Crash window**: There is an unavoidable window of vulnerability between the moment Discord accepts and publishes a message (HTTP 200 OK) and the moment the local worker commits `mark_succeeded()` to PostgreSQL or in-memory storage.
- If the host worker process is killed (`SIGKILL`, power failure, hardware fault) during this window:
  - The heartbeat task terminates immediately with the process.
  - The job remains in `status = 'processing'` with the last `locked_at` timestamp.
  - After `60 seconds` elapsed from `locked_at`, subsequent polling workers identify the job as a stale lease (`locked_at < stale_before`) and safely reclaim it for redelivery.
- The delivery audit log records `delivery_worker_id`, `attempts`, and error diagnostics for every transition to facilitate deduplication analysis.

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

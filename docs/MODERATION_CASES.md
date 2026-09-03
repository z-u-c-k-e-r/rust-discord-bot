# Moderation cases

The moderation case subsystem is the durable foundation for warnings, manual
sanctions, automated moderation evidence, appeals and later staff analytics.

## Trust boundary

Lua defines policy. Rust owns persistence and privileged execution.

Lua may declare:

- the target user;
- case type, reason, points, expiry and bounded metadata;
- a strictly increasing list of escalation thresholds;
- timeout, kick or ban as the action attached to a threshold.

Rust independently validates the action, Discord permissions, guild context and
role hierarchy. Lua never receives database credentials, raw SQL access, the bot
token or a privileged Discord client.

## Lifecycle

1. A moderator invokes `/warn` or another Lua module emits
   `create_moderation_case`.
2. Rust validates the request and persists an open case.
3. Active points are calculated from open cases that have not expired.
4. The highest matching escalation rule is applied after another permission and
   hierarchy check.
5. Creation and escalation are written to the audit log.
6. Staff can inspect cases through `/warnings` or the authenticated dashboard
   API.
7. `/case-resolve` or the CSRF-protected dashboard endpoint closes a case and
   removes its points from future escalation totals.

Expired cases remain available as evidence but no longer contribute points.
Resolved cases remain immutable historical records with the resolving user,
resolution and timestamp.

## Bundled commands

### `/warn`

Creates a warning with 1–10 points and an optional expiry. The module's
configuration defines timeout, kick and ban thresholds.

### `/warnings`

Lists up to 25 cases for a user. By default only open cases are returned;
moderators may include resolved history.

### `/case-resolve`

Closes an open case with a mandatory resolution. Closing a case is audited and
immediately removes its points from the active total.

### `/moderate`

Timeout, kick and ban actions create a zero-point case before the Discord
sanction is attempted. Purge remains an audit-only channel action.

## Configuration

The bundled module exposes:

- `warning_expiry_days`;
- `timeout_at_points`;
- `escalation_timeout_seconds`;
- `kick_at_points`;
- `ban_at_points`;
- `ban_delete_message_days`.

Thresholds are sanitized by the Lua module and validated again by Rust. A
misconfigured third-party module cannot bypass permission or hierarchy checks.

## Next extensions

The same records can support evidence attachments, staff notes, appeal threads,
automatic expiry jobs, reason templates, moderator performance metrics and
cross-module AutoMod incidents without changing the Lua trust boundary.

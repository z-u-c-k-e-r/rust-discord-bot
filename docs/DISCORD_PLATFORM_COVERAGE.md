# Discord platform capability coverage

Audit date: **2026-09-02**

This document defines the current Discord-platform scope for ZuckerBot. It is deliberately broader than a list of MEE6 commands: every supported Discord App, Bot, interaction, guild-management, community and monetization surface is either implemented, accepted into the roadmap, or explicitly marked as conditional/excluded.

The runtime targets Discord HTTP/Gateway API v10 and must be reviewed against Discord's changelog before each release.

## Status legend

- **Implemented** — connected end to end through Discord, Rust, Lua, storage/dashboard where relevant, authorization, audit and tests.
- **Foundation** — a reusable primitive exists, but the complete product flow is not finished.
- **Planned** — accepted product scope.
- **Conditional** — available only after policy, platform eligibility, licensing, privacy or a separate product decision.
- **Excluded** — intentionally unsupported because it would violate platform rules, user safety, law or the Rust/Lua trust boundary.

## Definition of complete

A capability is not complete merely because a Discord library exposes an endpoint. Every production feature must include:

1. a versioned Lua manifest/API where server behavior is configurable;
2. a capability-scoped Rust action or event contract;
3. validation of all IDs, lengths, URLs, files and Discord limits;
4. actor, bot and role-hierarchy authorization where applicable;
5. persistent per-guild configuration and state where applicable;
6. dashboard configuration with server-side guild authorization;
7. structured audit records for privileged or billable actions;
8. rate-limit-aware retries, idempotency and failure reporting;
9. unit, integration and authorization-boundary tests;
10. documentation of required OAuth2 scopes, intents and permissions.

## 1. App installation, identity and distribution

| Discord capability | Status | ZuckerBot delivery |
| --- | --- | --- |
| Guild/server installation | Implemented | Bot and `applications.commands` install flow documented. |
| User installation | Planned | Commands usable across a user's servers, bot DMs and private channels. |
| Guild plus user dual-install commands | Planned | Explicit `integration_types` per command. |
| OAuth2 authorization-code login | Implemented | Dashboard login, state validation and short-lived access-token use. |
| OAuth2 install links and default install settings | Planned | Generated from required scopes and least-privilege permissions. |
| Bot profile and current-member profile management | Planned | Avatar, banner, bio and per-guild nickname controls. |
| App Directory readiness | Planned | Install metadata, support/privacy links, discovery assets and verification gate. |
| Application event webhooks | Planned | Signature verification, replay defense and dead-letter handling. |
| Role Connections / Linked Roles | Planned | External-account metadata provider and verification workflow. |
| Team/application administration automation | Conditional | Developer Portal operations remain human-controlled unless Discord exposes a safe supported API. |

## 2. Application commands and interaction contexts

| Discord capability | Status | ZuckerBot delivery |
| --- | --- | --- |
| Chat-input slash commands | Implemented | Lua manifests compile to Discord commands. |
| User context-menu commands | Planned | Typed target-user context passed to Lua. |
| Message context-menu commands | Planned | Typed target-message snapshot with privacy controls. |
| Primary Entry Point command | Conditional | Only for a separately approved Discord Activity. |
| Guild interaction context | Implemented | Current command path. |
| Bot-DM interaction context | Foundation | DM permission exists; modern `contexts` model still required. |
| Private channel/GDM context | Planned | Requires user installation and privacy-specific action rules. |
| `integration_types` support | Planned | Guild-install, user-install or both, declared in Lua manifests. |
| `contexts` support | Planned | `GUILD`, `BOT_DM`, `PRIVATE_CHANNEL`, declared per command. |
| Command subcommands and groups | Implemented | Manifest nesting and Discord limits validated. |
| String, integer, number and boolean options | Foundation | Generic option model exists; full edge-case tests remain. |
| User, channel, role and mentionable options | Foundation | IDs remain strings at Lua/JSON boundaries. |
| Attachment command option | Planned | File metadata, size/type policy and isolated processing. |
| Static choices | Implemented | Up to Discord's per-option limit. |
| Dynamic autocomplete | Planned | Separate time-budgeted Lua handler and cache. |
| Command localization | Planned | Names/descriptions and dashboard language packs. |
| Default member permissions | Foundation | Permission data exists; manifest declaration and registration remain. |
| Per-guild command permission overrides | Planned | OAuth2 bearer-token flow and dashboard editor. |
| NSFW command declaration | Planned | Explicit manifest field and channel validation. |
| Immediate interaction responses | Implemented | Lua `reply` action. |
| Ephemeral responses | Implemented | Supported by the reply action. |
| Deferred responses | Planned | Required for slow storage, media and external APIs. |
| Follow-up messages | Planned | Token lifetime, ownership and expiry validation. |
| Original response edit/delete | Planned | Interaction-token-aware actions. |
| Autocomplete interactions | Planned | Strict response-time budget and no privileged side effects. |

## 3. Discord-native components and forms

ZuckerBot will support both legacy components and Components V2. Components are declarative Lua values; Rust owns custom-ID signing, expiry, authorization and interaction routing.

| Component or interaction | Status |
| --- | --- |
| Buttons, including link and premium buttons | Planned |
| String select | Planned |
| User select | Planned |
| Role select | Planned |
| Mentionable select | Planned |
| Channel select | Planned |
| Action Row | Planned |
| Section | Planned |
| Text Display | Planned |
| Thumbnail | Planned |
| Media Gallery | Planned |
| File display | Planned |
| Separator | Planned |
| Container | Planned |
| Modal Text Input | Planned |
| Modal Label | Planned |
| Modal File Upload | Planned |
| Modal Radio Group | Planned |
| Modal Checkbox Group | Planned |
| Modal Checkbox | Planned |
| Component interaction events | Planned |
| Modal-submit events | Planned |
| Signed custom IDs with expiration | Planned |
| Per-user/per-role component authorization | Planned |
| Persistent and restart-safe component sessions | Planned |
| Component pagination and wizard state | Planned |

Components V2 messages require their dedicated message flag and use component content instead of legacy `content`/`embeds`. The serializer must enforce the current Discord component-count and nesting rules rather than hard-code historic limits.

## 4. Messages, content and channels

| Discord capability | Status | ZuckerBot delivery |
| --- | --- | --- |
| Send and reply with plain text | Implemented | Empty allowed-mentions policy by default. |
| Edit bot messages | Planned | Ownership and stale-message handling. |
| Delete messages | Implemented | Permission checked in Rust. |
| Bulk delete/purge | Implemented | Current command capped at 100. |
| Rich embeds | Planned | Length, URL, media-host and mention validation. |
| Attachments and multipart uploads | Planned | Quotas, MIME sniffing, malware scan hook and isolated storage. |
| Reactions | Planned | Add/remove/list and reaction-driven Lua events. |
| Pins | Planned | Uses the dedicated `PIN_MESSAGES` permission introduced by Discord. |
| Suppress embeds/notifications | Planned | Explicit message flags only. |
| TTS messages | Conditional | Disabled by default; permission and abuse controls required. |
| Stickers in messages | Planned | Guild expression permission checks. |
| Native Discord polls | Planned | Create, close, inspect and vote-event handling. |
| Crosspost announcements | Planned | News-channel validation and audit. |
| Typing indicator | Planned | Only for bounded operations that genuinely need it. |
| Webhooks | Planned | Create/edit/delete/execute, token isolation and signed inbound triggers. |
| Followed announcement channels | Planned | Create and audit follow relationships. |
| Text/announcement/category channels | Planned | Full create/edit/delete/clone and permission-overwrite support. |
| Voice and Stage channels | Foundation | Voice joining exists; management actions remain. |
| Public, private and announcement threads | Planned | Create, join, archive, lock and membership workflows. |
| Forum channels and posts | Planned | Tags, default reactions, moderation and ticket/LFG adapters. |
| Media channels and posts | Planned | Feature-gated while Discord marks the API as actively evolving. |
| Channel permission overwrites | Planned | Diff preview, lockout prevention and rollback. |
| Slowmode | Planned | Includes the modern `BYPASS_SLOWMODE` permission model. |
| Channel invites | Planned | Attribution, expiry/use limits and revocation. |
| Group-DM recipient management | Excluded | Requires user tokens/scopes and does not belong in the server-bot control plane. |

## 5. Guild administration and configuration

| Discord capability | Status |
| --- | --- |
| Guild information and settings inspection | Planned |
| Channel/category create, update, reorder and delete | Planned |
| Role create, update, reorder and delete | Planned |
| Add/remove existing member roles | Implemented |
| Member nickname management | Planned |
| Current bot member nickname/profile management | Planned |
| Kick, ban and timeout | Implemented |
| Unban, temporary ban and soft ban | Planned |
| Bulk bans with raid safeguards | Planned |
| Member prune preview/execution | Planned |
| Member search and role member counts | Planned |
| Guild audit-log ingestion | Planned |
| Audit-log reasons on eligible actions | Foundation |
| Guild widget settings/data | Planned |
| Welcome Screen management | Planned |
| Community Onboarding management | Planned |
| Rules/membership-screening assistance | Planned |
| Guild Scheduled Events | Planned |
| Recurring Scheduled Events | Planned |
| Event subscriber/attendance workflows | Planned |
| Guild templates | Planned |
| Server backup/export and controlled restore | Planned |
| Guild integrations inspection | Planned |
| Vanity URL inspection | Planned |
| Emoji create/update/delete | Planned |
| Sticker create/update/delete | Planned |
| Soundboard list/send/create/update/delete | Planned |
| Stage Instance create/update/delete | Planned |
| Stage speaker/request-to-speak moderation | Planned |

## 6. Members, roles and community lifecycle

| Discord capability | Status |
| --- | --- |
| Member join event | Implemented |
| Welcome message | Implemented |
| Member update/leave/ban events | Planned |
| Autoroles and delayed roles | Planned |
| Role menus and reaction roles | Planned |
| Temporary and persistent roles | Planned |
| Role synchronization across guilds | Planned |
| Account-age and membership-age gates | Planned |
| Invite attribution | Planned |
| Presence/status/activity events | Planned; privileged intent where required |
| Voice-state events | Planned |
| User/member/profile information commands | Planned |
| Avatar/banner display | Planned |
| Birthday and anniversary workflows | Planned |
| Reputation/thanks | Planned |
| LFG and event sign-ups | Planned |
| Suggestions, voting and starboard | Planned |
| Giveaways with auditable winner selection | Planned |
| Native polls and anonymous application polls | Planned |
| Levels, text XP and voice XP | Planned |
| Rank cards, rewards and leaderboards | Planned |
| Virtual economy and inventory | Planned |
| Server-specific achievements | Planned |

## 7. Moderation, AutoMod and trust & safety

| Discord capability | Status |
| --- | --- |
| Timeout, kick, ban and purge actions | Implemented |
| Actor permission and hierarchy validation | Implemented |
| Privileged-action audit records | Implemented |
| Lua blocked-word example | Implemented |
| Warnings, notes and numbered cases | Planned/Foundation |
| Native AutoMod rule list/create/update/delete | Planned |
| Native AutoMod execution events | Planned |
| Keyword, regex, preset and mention-spam rules | Planned |
| Block Message action | Planned |
| Send Alert Message action | Planned |
| AutoMod Timeout action | Planned |
| Block Member Interaction action | Planned |
| Anti-spam and duplicate-message detection | Planned |
| Mention, emoji, caps and Unicode flood protection | Planned |
| Invite/domain/attachment filters | Planned |
| Join-velocity anti-raid | Planned |
| New-account and suspicious-bot gates | Planned |
| Quarantine, channel/category lockdown | Planned |
| Evidence snapshots with retention controls | Planned |
| Modmail, appeals and moderator queue | Planned |
| Shared ban intelligence | Conditional | Requires provenance, privacy, appeal and false-positive controls. |
| Automated AI enforcement | Conditional | Human-review policy required; no autonomous ban by default. |

## 8. Voice, music, Stage and soundboard

| Discord capability | Status |
| --- | --- |
| Join invoking member's voice channel | Implemented |
| Search/HTTPS source resolution | Implemented |
| Play queue, pause, resume, skip, stop and leave | Implemented |
| Source-host allowlist | Implemented |
| Now playing and queue pagination | Planned |
| Remove/move/shuffle/repeat tracks | Planned |
| Volume and audio filters | Planned |
| DJ role, vote skip and channel ownership | Planned |
| Saved playlists, favorites and history | Planned |
| Radio streams and 24/7 mode | Planned |
| Dedicated distributed voice nodes | Planned |
| Reconnect/resume and node failover | Planned |
| Stage Instance management | Planned |
| Soundboard playback and management | Planned |
| Text-to-speech | Conditional | Abuse, provider cost and content policy controls required. |
| Voice receive/recording/transcription | Conditional | Explicit consent, jurisdiction, retention and deletion design required. |
| Lyrics/karaoke display | Conditional | Licensing required. |

Music adapters must respect source-platform terms and copyright rules. ZuckerBot will not implement access-control bypasses or media DRM circumvention.

## 9. Gateway events, intents and scale

| Discord capability | Status |
| --- | --- |
| Gateway connection and heartbeat handling | Implemented through Serenity |
| Session resume/reconnect | Implemented through Serenity; operational tests planned |
| Guild events | Foundation |
| Message-create event | Implemented |
| Member-add event | Implemented |
| Message update/delete/bulk-delete events | Planned |
| Reaction events | Planned |
| Channel/thread events | Planned |
| Guild/member/role events | Planned |
| Invite/integration/webhook events | Planned |
| Voice-state/server/effect events | Planned |
| Stage and Scheduled Event events | Planned |
| AutoMod configuration/execution events | Planned |
| Poll vote events | Planned |
| Entitlement/subscription events | Planned |
| Soundboard events | Planned |
| Presence and typing events | Planned/optional |
| Minimal-intent calculation from enabled modules | Planned |
| Privileged-intent diagnostics | Planned |
| Sharding | Foundation through Serenity |
| Multi-process shard supervisor | Planned |
| Identify concurrency/session-start accounting | Planned |
| Distributed rate-limit coordination | Planned |
| Backpressure and bounded event queues | Planned |
| Idempotent event processing | Planned |
| Dead-letter queue and replay tooling | Planned |

Discord rate-limit values are treated as dynamic. The platform must follow response headers and library state rather than hard-code historic limits.

## 10. Dashboard and control plane

| Capability | Status |
| --- | --- |
| Discord OAuth2 login | Implemented |
| Manageable-guild filtering | Implemented |
| Per-guild module list | Implemented |
| Module enable/disable | Implemented |
| Raw JSON configuration editor | Implemented |
| CSRF-protected writes | Implemented |
| Configuration audit | Implemented |
| Schema-generated visual forms | Planned |
| Setup wizard | Planned |
| Permission and intent diagnostics | Planned |
| Command/channel/role policy editor | Planned |
| Moderation cases and appeals | Planned |
| Ticket configuration and queue | Planned |
| Music/player controls | Planned |
| Role-menu and workflow builder | Planned |
| Scheduled jobs calendar | Planned |
| Logs and searchable audit trail | Planned |
| Analytics and report exports | Planned |
| Data retention/privacy center | Planned |
| Dashboard organization roles | Planned |
| Multi-guild organization view | Planned |
| Real-time updates via WebSocket/SSE | Planned |
| Mobile-first administration | Foundation |
| API tokens and scoped service accounts | Planned |
| Outgoing webhooks | Planned |
| Backup, import, diff and rollback | Planned |

## 11. Lua platform coverage

Every product feature should be scriptable without making Lua privileged infrastructure code.

| Lua platform capability | Status |
| --- | --- |
| Versioned module manifest | Implemented |
| Slash-command declaration | Implemented |
| Event subscription declaration | Implemented |
| Per-guild JSON configuration | Implemented |
| Typed command/event context | Implemented |
| Declarative action return values | Implemented |
| Fresh VM per invocation | Implemented |
| Memory and global instruction limits | Implemented |
| No filesystem, process, network or database access | Implemented |
| Action-count limit | Implemented |
| Runtime manifest/action validation | Implemented |
| Capability declarations per module | Planned |
| Versioned Lua API negotiation | Foundation |
| Component and modal declarations | Planned |
| Scheduler handlers | Planned |
| Autocomplete handlers | Planned |
| Durable namespaced key/value state | Foundation |
| Transactional state changes | Planned |
| Secret references without secret disclosure | Planned |
| External connector actions through allowlisted Rust adapters | Planned |
| Package signing and provenance | Planned |
| Dependency graph and semantic version constraints | Planned |
| Staged rollout, health check and rollback | Planned |
| Module marketplace and review pipeline | Planned |
| Lua language-server annotations and SDK | Planned |
| Deterministic test harness and event fixtures | Planned |
| CPU/memory/action quota observability | Planned |

## 12. Monetization and premium features

Discord supports monetization for Bots and Activities through SKUs, Entitlements and Subscriptions.

| Discord capability | Status |
| --- | --- |
| User subscription SKU | Planned |
| Guild subscription SKU | Planned |
| Durable one-time purchase | Planned |
| Consumable one-time purchase | Conditional | Requires transaction ledger and exactly-once consumption. |
| Entitlement create/update/delete events | Planned |
| Subscription events | Planned |
| Entitlement checks on interactions | Planned |
| Premium buttons and Store links | Planned |
| Test entitlements/application test mode | Planned |
| Premium feature gates in Rust and Lua context | Planned |
| Billing audit and reconciliation | Planned |
| Grace periods/refund/revocation handling | Planned |
| Tax, business and platform-eligibility setup | Conditional | Operational/legal prerequisite, not bot code. |

Lua may inspect a sanitized entitlement view and request premium actions, but Rust remains authoritative for access decisions and consumption.

## 13. External integrations and optional modules

Accepted optional module families include:

- RSS/Atom, YouTube, Twitch, Reddit and social notifications;
- GitHub repositories, releases, issues and deployment events;
- calendars, reminders, status pages and incident systems;
- game-server status, Steam, Minecraft, Path of Exile and Hero Siege;
- translation, weather, time zones and unit/currency conversion;
- image/meme generation and resource-limited file conversion;
- ticketing, CRM and knowledge-base providers;
- privacy-reviewed AI assistance for search, summaries and drafting.

External services are never called directly from Lua. Each provider receives a Rust adapter with an allowlist, OAuth/token isolation, quotas, timeout, retry policy, audit behavior and per-guild consent.

## 14. Separate Discord products

The following are Discord platform capabilities but are not ordinary bot endpoints. They require separate architecture and release tracks:

| Platform product | Status |
| --- | --- |
| Discord Activities / Embedded App SDK | Conditional separate service |
| Primary Entry Point Activity launch | Conditional separate service |
| Activity instance participation | Conditional separate service |
| Activity in-app purchases | Conditional separate service |
| Discord Social SDK for games | Out of bot runtime scope |
| Rich Presence/Game SDK integration | Optional game-integration project |

They may share ZuckerBot identity, OAuth and entitlements, but they must not be placed inside the Gateway bot process.

## 15. Explicitly unsupported behavior

ZuckerBot will not implement:

- self-bots or automation through normal user accounts;
- token harvesting, credential capture or session theft;
- spam, unsolicited mass DMs or mention abuse;
- attempts to evade Discord rate limits, verification or moderation;
- destructive actions without permission, hierarchy and audit controls;
- unrestricted shell, filesystem, SQL or network access from Lua;
- copyrighted media access bypasses or DRM circumvention;
- covert voice recording or biometric profiling;
- gambling features before a jurisdiction-specific legal and age-gating review.

## Current 2026 compatibility notes

The implementation roadmap must account for current Discord behavior, not historic examples:

- API v10 remains the target available API version.
- Commands use `integration_types` and `contexts`; the older `dm_permission` field is deprecated.
- User and Message commands are first-class context-menu surfaces, and Entry Point commands belong to Activities.
- Components V2 introduces layout/content components and modern modal inputs including File Upload, Radio Group and Checkbox controls.
- Components V2 messages have different payload rules from legacy `content`/`embeds` messages.
- `PIN_MESSAGES`, `BYPASS_SLOWMODE`, `CREATE_GUILD_EXPRESSIONS` and `CREATE_EVENTS` must be checked as distinct permissions under Discord's 2026 permission model.
- Application flags may require the string-serialized `flags_new` field for bits beyond the legacy 32-bit field.
- Native AutoMod, poll-vote and soundboard events have dedicated Gateway intents/events.
- Forum and Media channels are thread-only; Media channel behavior must remain feature-gated while Discord labels it actively evolving.
- Rate limits are discovered from Discord responses and must not be hard-coded.

## Delivery gate

The accepted scope is intentionally exhaustive, but implementation remains incremental. A release or marketing page may only list entries marked **Implemented**. The dashboard should eventually expose this same distinction so server owners never configure a placeholder as though it were functional.

# Discord capability and ZuckerBot product catalog

Last reviewed: 2026-09-02

This document defines the complete target surface for ZuckerBot. It separates capabilities exposed by the official Discord platform from higher-level product modules built on top of them. A feature is not considered implemented merely because it appears in this catalog; implementation status belongs in `FEATURE_MATRIX.md` and must be backed by tests.

## Product rule

Server-specific behavior is authored in versioned Lua 5.4 modules. Rust remains the trusted control plane and is solely responsible for Discord credentials, Gateway and REST traffic, authorization, role hierarchy checks, rate limits, durable storage, secrets, voice transport, audit records and execution of privileged actions.

“Everything” means every relevant capability available to compliant Discord applications plus the expected all-in-one community features. It explicitly excludes self-bot behavior, user-token automation, client modification, permission bypasses, scraping private data and any action forbidden by Discord policy.

## Official Discord platform surfaces

ZuckerBot must provide safe abstractions for all relevant public application surfaces:

- bot and user installation contexts, OAuth2 authorization and guild selection;
- Gateway sessions, resumability, heartbeats, privileged intents and automatic sharding;
- global and guild application commands: chat-input, user and message commands;
- interactions, autocomplete, deferred responses, follow-ups and ephemeral responses;
- buttons, select menus, text inputs, modals and Components V2 layouts;
- messages, replies, embeds, attachments, stickers, polls, reactions and mentions;
- channels, categories, threads, forum posts, tags, permissions and overwrites;
- guild members, roles, nicknames, onboarding, welcome screens and verification flows;
- moderation actions, timeouts, bans, audit-log correlation and Discord AutoMod rules;
- voice connections, stages, speaking state and soundboard-related events where available;
- webhooks, interaction webhooks and external integration callbacks;
- invites, emojis, stickers, templates and scheduled events;
- role connections and linked-role metadata;
- SKUs, entitlements and subscription events for optional premium capabilities;
- rate-limit buckets, permission calculations, allowed mentions and payload validation;
- localization for commands, responses and dashboard configuration.

Primary references:

- https://discord.com/developers/docs/intro
- https://discord.com/developers/docs/topics/gateway
- https://discord.com/developers/docs/interactions/overview
- https://discord.com/developers/docs/interactions/application-commands
- https://discord.com/developers/docs/components/overview
- https://discord.com/developers/docs/resources/auto-moderation
- https://discord.com/developers/docs/topics/voice-connections
- https://discord.com/developers/docs/topics/oauth2
- https://discord.com/developers/docs/monetization/overview

## Complete product module inventory

### 1. Moderation and case management

- warn, note, timeout, remove-timeout, kick, soft-ban, ban, unban and mass-ban;
- temporary actions with durable expiry jobs and restart-safe execution;
- filtered purge by author, bot, attachment, embed, link, regex and time range;
- channel slowmode, lock, unlock, hide, reveal and server-wide emergency lockdown;
- nickname and role corrections with Discord role-hierarchy enforcement;
- structured cases, evidence, attachments, internal notes and immutable action history;
- configurable reason templates, escalation ladders and repeat-offender policies;
- moderator assignment, case search, exports and dashboard review queues;
- modmail, appeals, appeal decisions and configurable appeal cooldowns;
- optional federated deny lists only with explicit governance, provenance and appeal rules.

### 2. AutoMod, anti-raid and trust

- spam rate, duplicate text, mention spam, emoji spam, caps and newline flooding;
- invite, URL, domain, phishing, scam phrase and malicious attachment policies;
- Unicode confusable, zero-width character and obfuscated-link normalization;
- per-channel, per-role and per-user exemptions with auditable precedence;
- warn, delete, quarantine, timeout, kick, ban, alert and challenge actions;
- join-velocity detection, raid windows, account-age gates and bot-add protection;
- verification levels, web challenge flows, one-time codes and manual review queues;
- trusted-member progression, probation roles and risk-based restrictions;
- emergency mode with reversible channel and role snapshots;
- synchronization with native Discord AutoMod where its rule model is sufficient.

### 3. Audit logging and compliance

- message create/edit/delete/bulk-delete metadata with configurable content retention;
- member join/leave/update, role, nickname, timeout and moderation events;
- channel, permission, webhook, invite, emoji, sticker, thread and forum changes;
- voice join/leave/move/mute/deafen and temporary-channel ownership changes;
- scheduled-event, onboarding and server-configuration changes;
- dashboard, API, automation and Lua privileged-action audit trails;
- actor correlation with Discord audit logs where Discord exposes the data;
- retention policies, redaction, legal hold, JSON/CSV export and deletion workflows;
- tamper-evident event identifiers and append-only security records.

### 4. Tickets, forms, applications and support

- button/select ticket panels with multiple departments and routing policies;
- private channels or threads, participant management, claim/unclaim and transfer;
- forms, modal questions, validation, conditional fields and reusable templates;
- priorities, tags, SLA timers, reminders, escalation and staff availability rules;
- transcripts in HTML/JSON, attachment manifests and configurable retention;
- close/reopen/delete flows, close reasons, ratings and support analytics;
- staff, guild, partnership, ban-appeal and whitelist application workflows;
- multi-step approvals, voting, reviewer notes and outbound webhooks.

### 5. Roles, onboarding and member lifecycle

- autoroles, delayed roles, temporary roles and role restoration after rejoin;
- reaction, button and select-menu role panels;
- unique, required, mutually exclusive and limited-capacity role groups;
- pronoun, region, platform, game, notification and access role menus;
- welcome/goodbye messages, images, DMs and multi-step onboarding sequences;
- rules acknowledgement, verification, screening reminders and staff alerts;
- member milestones, anniversaries, birthdays and inactivity policies;
- join questions, interest routing and personalized channel recommendations;
- role synchronization through approved external account connections.

### 6. Levels, reputation, achievements and economy

- message, voice, event and task XP with anti-farm controls;
- configurable curves, prestige/master levels and seasonal resets;
- role rewards, unlocks, badges, achievements and milestone announcements;
- global, server, role, channel, friend and seasonal leaderboards;
- reputation/thanks with cooldowns and abuse detection;
- currencies, wallets, banks, daily/weekly rewards, streaks and taxes;
- shops, inventories, consumables, collectibles, crafting and trading;
- quests, bounties, community goals, auctions and configurable sinks;
- complete transaction ledger and administrator rollback tools.

### 7. Music, audio and voice entertainment

- play, search, pause, resume, stop, seek, skip, previous and disconnect;
- persistent queue, remove, move, clear, shuffle, loop track/queue and autoplay;
- playlists, favorites, history, saved queues and restart recovery;
- volume, normalization, equalizer and supported real-time audio filters;
- DJ roles, vote skip, requester rules, channel binding and duplicate controls;
- track metadata, chapters, thumbnails, live-stream state and lyrics links where licensed;
- 24/7 mode, stage-channel support and distributed audio workers;
- pluggable source providers with explicit allowlists and policy-compliant extraction;
- no DRM bypass, credential theft or unsupported redistribution.

### 8. Temporary voice channels

- join-to-create hubs, private/public rooms and category-specific templates;
- owner transfer, trust/block lists, permit/reject, lock/unlock and hide/reveal;
- rename, user limit, bitrate, region and activity-aware naming;
- text companion channels, control panels and automatic cleanup;
- scheduled rooms, party/lobby matching and role-based limits;
- voice activity analytics and XP with privacy-preserving aggregation;
- recording only as an explicit, jurisdiction-aware, all-participant-consent feature.

### 9. Community engagement

- starboard/highlight boards with weighted and role-aware thresholds;
- suggestions with voting, states, staff responses and changelog publishing;
- polls, ranked votes, anonymous votes and scheduled result closure;
- giveaways with requirements, weighted entries, rerolls and fraud controls;
- counting, word chain, quote board, confession, question of the day and bump reminders;
- birthdays, events, RSVPs, attendance, recurring events and reminders;
- trivia, quizzes, achievements, tournaments and lightweight chat games;
- server partnerships, introductions, profiles and opt-in matching.

### 10. Fun, media and memes

- meme templates, image captions, GIF search and reaction media;
- avatar, banner, server icon and profile-card rendering;
- quotes, jokes, facts, randomizers, dice, coin, choose and eight-ball;
- image transformations with strict size, timeout and content controls;
- safe provider adapters for external media catalogs;
- administrator-configurable content ratings and channel restrictions.

### 11. Utilities and productivity

- user, member, role, channel, server, emoji, invite and permission inspection;
- reminders, timers, recurring schedules, time zones and timestamp generation;
- calculator, unit conversion, color, hash, encoding and text utilities;
- translation, dictionary, weather and other provider-backed tools through adapters;
- embed/message builder with previews, templates and scheduled publishing;
- polls, announcements, sticky messages, FAQ and self-service knowledge commands;
- backups of ZuckerBot-owned configuration and reversible setup templates.

### 12. Automation and custom workflows

- trigger/action builder exposed in Lua and through a visual dashboard editor;
- Gateway-event, command, schedule, webhook, feed and manual triggers;
- conditions over roles, channels, time, configuration and namespaced module state;
- branches, delays, retries, timeouts, idempotency keys and compensation actions;
- reusable variables, templates, localization and typed configuration schemas;
- allowlisted HTTP connector actions with SSRF protection and response size limits;
- namespaced durable key/value, counters, sets, queues and scheduled jobs;
- simulation, dry-run, trace view and rollback-safe deployment of workflow versions.

### 13. Notifications and integrations

- YouTube, Twitch, RSS/Atom, GitHub, release and status-page notifications;
- Steam/game-server presence and approved game API integrations;
- calendars, forms, issue trackers and CI/CD notification adapters;
- incoming/outgoing webhooks, signatures, retries and dead-letter queues;
- customizable announcement templates, mention policies and deduplication;
- health checks and credential-expiry warnings in the dashboard.

### 14. AI-assisted modules

- opt-in FAQ and knowledge assistant grounded in administrator-selected sources;
- thread/channel summaries, translation and accessibility assistance;
- moderation triage that recommends but does not silently invent evidence;
- semantic search over explicitly retained server knowledge;
- text/image provider adapters with quotas, cost controls and safety policies;
- per-guild consent, data minimization, deletion and provider disclosure;
- no training-data claim or private-message ingestion without explicit authorization.

### 15. Analytics and insights

- joins, leaves, retention, active members and cohort trends;
- messages, reactions, threads, voice, events and command usage;
- moderation volume, response time, repeat offenses and AutoMod precision review;
- ticket SLA, resolution, satisfaction and staff workload;
- level/economy health, giveaway integrity and integration delivery status;
- privacy-aware aggregation, configurable retention and CSV/JSON export;
- dashboard charts plus a metrics API for authorized administrators.

### 16. Dashboard and control plane

- Discord OAuth2 login and server picker;
- live verification of user guild permissions and ZuckerBot access;
- owner/admin/custom RBAC roles with least-privilege scopes;
- schema-generated forms for every Lua module;
- draft, validate, preview, publish, rollback and version history;
- audit log, active sessions, CSRF protection and session revocation;
- secrets vault references, never plaintext secrets returned to Lua or the browser;
- module enablement, dependency resolution, health and execution metrics;
- responsive desktop/mobile UI, accessibility and localization;
- import/export, environment promotion and disaster-recovery controls.

### 17. Lua module platform and marketplace

- signed module manifest with ID, semantic version, SDK range and dependencies;
- declared commands, events, jobs, configuration schema and capabilities;
- isolated execution budget for memory, instructions, wall time and action count;
- namespaced storage, migrations and optimistic concurrency;
- capability-scoped Discord actions validated by Rust;
- allowlisted connector calls rather than direct arbitrary sockets;
- hot reload through staged compile, test and atomic activation;
- deterministic event fixtures, unit tests and integration simulator;
- module logs, traces, metrics and structured error reporting;
- trusted first-party registry and optional reviewed third-party marketplace;
- automatic rollback after repeated failures or policy violations.

### 18. Developer API and extensibility

- documented Lua SDK and generated type stubs;
- REST/OpenAPI control API with scoped service credentials;
- outbound event webhooks with signing and replay protection;
- command/event test harness and local development environment;
- module scaffolding, formatter, linter, package validator and test runner;
- migration tooling, fixtures and reproducible development containers;
- stable audit/action envelopes with explicit API versioning.

### 19. Reliability and operations

- automatic Discord sharding and horizontal worker scaling;
- PostgreSQL for durable state, Redis for coordination/cache and object storage for artifacts;
- durable queues, retries, dead-letter handling and idempotent jobs;
- per-guild fairness, backpressure and Discord rate-limit coordination;
- graceful shutdown, session resume, queue draining and rolling deployments;
- liveness, readiness, metrics, traces, structured logs and alerting;
- encrypted backups, restore drills, schema migration gates and rollback plans;
- regional deployment options and data-retention controls;
- dependency, license, secret and vulnerability scanning in CI.

### 20. Optional monetization

- Discord entitlement-aware premium feature gates;
- subscriptions, trials, grace periods and server/user ownership rules;
- transparent quotas, usage metering and administrator-visible billing state;
- no moderation or data-export lock-in; security-critical controls remain available;
- open-source self-hosted edition remains a first-class deployment target.

## Lua security contract

Lua modules never receive the Discord token, OAuth refresh tokens, database credentials or provider secrets. The standard `os`, unrestricted `io`, native module loading, FFI, process execution and arbitrary network access are unavailable. Modules receive immutable event DTOs and a capability-scoped SDK, then return declarative actions.

Rust validates every action for:

1. module capability grant;
2. guild and channel scope;
3. invoking user authorization where applicable;
4. bot permissions and Discord role hierarchy;
5. payload constraints and allowed mentions;
6. rate limits, quotas and idempotency;
7. auditability and configured policy;
8. current resource state before execution.

## Definition of done for every catalog entry

A capability can be marked implemented only when it has:

- a versioned contract and configuration schema;
- authorization and negative-path tests;
- Lua sandbox and quota coverage where Lua is involved;
- integration tests for the Rust capability executor;
- dashboard validation and audit records for configuration changes;
- documented privacy, retention and provider behavior;
- observable metrics and structured errors;
- migration and rollback behavior;
- passing formatting, Clippy, tests and release build on the exact commit.

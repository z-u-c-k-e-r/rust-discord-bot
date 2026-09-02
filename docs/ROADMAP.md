# ZuckerBot product roadmap

The target is broader than a MEE6 clone: one Discord platform with a safe Lua SDK, first-party modules, a self-service dashboard and an extension ecosystem. This document separates the implemented foundation from planned work so repository claims remain verifiable.

## Status legend

- **Done** — present in the repository and intended to work now.
- **Next** — immediate production-hardening milestone.
- **Planned** — designed category, not yet implemented.
- **Research** — needs API, licensing, abuse or operating-cost validation.

## 0.1 — platform foundation

**Done**

- Rust 2024 workspace using Serenity, Songbird, mlua and Axum.
- Lua module discovery, manifest validation and atomic reload.
- Fresh sandbox per execution with memory, instruction and wall-clock limits.
- Global Discord slash commands generated from Lua manifests.
- Per-guild module allow-list and JSON configuration.
- Owner dashboard protected by a long Bearer secret.
- Typed action allow-list for replies, messages, kick, ban and music controls.
- Action-level permission checks in Rust.
- Basic modules: core, fun/memes, moderation and music.
- Docker image, hardened Compose service and CI.

## 0.2 — secure multi-user dashboard

**Next**

- Discord OAuth2 authorization-code flow with PKCE and state validation.
- Server-side encrypted sessions and secure cookies.
- CSRF protection for all mutations.
- Guild picker filtered to `MANAGE_GUILD` / `ADMINISTRATOR`.
- Staff roles with owner, administrator, moderator, analyst and read-only scopes.
- PostgreSQL migrations and row-level tenant boundaries.
- Redis-backed sessions, cache, rate limits and distributed locks.
- Immutable audit trail for dashboard, moderation and scripting changes.
- Secret rotation and optional passkey/MFA requirement for platform owners.
- Schema-driven forms generated from a Lua module's configuration definition.
- Live configuration validation and preview.

## 0.3 — moderation and safety suite

**Planned**

- Warnings, notes, strikes and configurable escalation ladders.
- Timeouts, soft bans, unbans, mass actions and message purge.
- Complete moderation case IDs, evidence, attachments and appeal status.
- Mod-log channels and configurable public/private reason templates.
- Anti-spam: duplicate content, mention floods, emoji floods and message velocity.
- Anti-raid: join velocity, account age, suspicious profile patterns and lockdown modes.
- Anti-nuke: destructive action thresholds, trusted-role bypass and emergency recovery.
- Link, invite, domain, attachment, filename and MIME policies.
- Unicode/confusable detection and phishing-domain intelligence.
- Discord AutoMod rule synchronization and native AutoMod event handling.
- Quarantine roles, verification gates and CAPTCHA provider adapters.
- Moderator action approval for high-impact operations.
- Evidence retention and privacy-aware deletion policies.
- Appeals portal and Discord-native appeal forms.

## 0.4 — onboarding, roles and identity

**Planned**

- Welcome and goodbye messages with embeds, images and per-channel routing.
- Autoroles, delayed roles and restore-on-rejoin roles.
- Button/select reaction roles with exclusivity, prerequisites and role limits.
- Role menus generated from Lua configuration schemas.
- Membership screening helpers and rule acceptance workflows.
- Verification flows for Steam, GitHub, Twitch, YouTube and game accounts.
- Nickname templates and synchronization policies.
- Temporary roles and expiring access grants.
- Birthday, anniversary and membership-duration roles.
- Cross-server identity and role federation for approved communities.

## 0.5 — community, XP and economy

**Planned**

- Message and voice XP with anti-farming cooldowns.
- Configurable XP formulas, channel/role multipliers and seasonal resets.
- Levels, ranks, prestige, achievements and role rewards.
- Public and private leaderboards with privacy controls.
- Reputation, thanks and endorsement systems.
- Virtual currency, daily/weekly rewards, inventory and shops.
- Role, cosmetic and temporary-perk purchases.
- Quests, streaks, collections, crafting and server events.
- Economy transaction ledger, fraud limits and rollback tools.
- Import tools for MEE6 and other common leveling exports where legally/API-permitted.

## 0.6 — music and voice platform

**Planned**

- Rich queue view, now-playing cards and interactive controls.
- Search selection, playlists, history, favorites and saved guild playlists.
- Loop modes, seek, shuffle, autoplay, vote skip and DJ roles.
- Per-user and per-role queue limits.
- Volume normalization, equalizer, filters and configurable audio quality.
- Spotify/Apple Music metadata resolution to playable sources where permitted.
- Radio streams, podcasts and approved direct media URLs.
- Regional voice workers, reconnect recovery and queue persistence.
- Stage channel support and scheduled radio shows.
- Text-to-speech with abuse controls.
- Voice activity statistics with consent and retention controls.

Music providers and extractors must be reviewed continuously for terms of service, copyright and technical stability. The architecture must allow a provider to be disabled without breaking the entire bot.

## 0.7 — tickets, forms and support operations

**Planned**

- Ticket panels using buttons and selects.
- Modal-based intake forms and conditional questions.
- Department routing, priorities, tags, assignments and SLA timers.
- Private thread or channel ticket backends.
- Claim, transfer, merge, close, reopen and escalation workflows.
- HTML/PDF/JSON transcripts with attachment references.
- Satisfaction surveys and support analytics.
- Knowledge-base suggestions and canned responses.
- User blocklists, cooldowns and abuse controls.
- External helpdesk connectors through approved adapters.

## 0.8 — content, feeds and integrations

**Planned**

- YouTube, Twitch, Kick, TikTok, Reddit, RSS/Atom and podcast notifications.
- GitHub releases, commits, issues, pull requests and Actions notifications.
- Steam news and game-server status adapters.
- Calendar and scheduled-event synchronization.
- Custom webhooks with signatures, retries and dead-letter queues.
- Message templates, embeds and localization.
- Announcement cross-posting, thread creation and role mentions.
- Polls, suggestions, starboard and highlight channels.
- Counting, confessions, anonymous feedback and quote boards.
- Meme APIs and image-generation providers behind configurable safety policies.
- Domain allow-list, request quotas and secret vault for integration credentials.

## 0.9 — events, games and engagement

**Planned**

- Giveaways with role/account-age/message/voice prerequisites.
- Rerolls, multi-winner selection, audit proofs and anti-alt controls.
- Polls, ranked-choice votes and anonymous ballots.
- Reminders, birthdays, timers and recurring schedules.
- Event sign-ups, waitlists, teams and attendance tracking.
- Discord Scheduled Event creation and synchronization.
- Trivia, word games, reaction games and tournament brackets.
- Temporary voice rooms and party finders.
- Looking-for-group posts with expiry and matching.
- Community challenges and season passes.

## 1.0 — analytics and operations

**Planned**

- Server growth, joins/leaves, retention and activation funnels.
- Message, thread, forum and voice activity metrics.
- Command adoption, errors and latency by module and guild.
- Moderation volume, response time and repeat-offender analysis.
- Configurable reports and scheduled exports.
- Privacy-respecting aggregation, configurable retention and data deletion.
- Prometheus metrics, OpenTelemetry traces and structured logs.
- Health dashboard, incident status and owner alerts.
- Automated backups, restore drills and disaster recovery runbooks.
- Shard orchestration, zero-downtime deployments and canary releases.
- Per-guild quotas, fair-use controls and subscription entitlements.

## 1.x — Lua extension ecosystem

**Planned**

- Versioned Lua SDK with semantic capability versions.
- Typed persistent key-value store and transactional operations.
- Timers, durable jobs, cron and event subscriptions.
- Components: embeds, buttons, selects, modals and autocomplete.
- Approved HTTP adapters with domain and method allow-lists.
- Module configuration schemas, migrations and localization bundles.
- Unit-test harness, event simulator and local developer CLI.
- Signed module packages, provenance and reproducible builds.
- Static analysis, permission declarations and automated security review.
- Private guild modules and a curated public marketplace.
- Rollback, staged rollout and per-guild version pinning.
- Resource quotas and module circuit breakers.

## AI-assisted features

**Research**

- Optional semantic moderation queues with human review.
- Support-answer drafting from an approved guild knowledge base.
- Thread summaries, translation and meeting summaries.
- Natural-language dashboard search and configuration explanations.
- Safe image/text generation with provider-specific policy enforcement.
- Cost budgets, opt-in consent, data minimization and no-training guarantees where available.

AI must never autonomously ban users based only on a model score. High-impact actions require deterministic rules or human approval, and every model-assisted decision must retain its evidence and configuration version.

## Non-goals and hard rules

- Lua never receives the Discord token or unrestricted OS/network/database access.
- The bot will not implement self-bot behavior or automate user accounts.
- The platform will not bypass Discord rate limits, permissions or safety systems.
- Music and external content support will respect applicable provider terms and copyright constraints.
- Privacy-sensitive analytics are opt-in/configurable and have deletion/retention controls.
- High-impact moderation actions are auditable and recoverable where Discord permits.

## Suggested delivery order

1. Make CI green and deploy 0.1 to one private test guild.
2. Add OAuth2, PostgreSQL, Redis, sessions, CSRF and audit logging.
3. Build moderation cases, logging and automod before public onboarding.
4. Add role/onboarding and ticket systems.
5. Separate regional voice workers and harden music providers.
6. Add XP/economy, feeds, events and analytics.
7. Stabilize the Lua SDK before opening an extension marketplace.
8. Add subscriptions only after quotas, observability, backups and deletion tooling are proven.

# Discord capability matrix

This matrix turns the product goal “everything a serious Discord bot can do” into explicit platform surfaces and modules. **Target** means the architecture must support it; it does not mean the feature is already implemented.

## Native Discord interaction surfaces

| Capability | Discord surface | 0.1 | Target |
|---|---|---:|---:|
| Slash commands | Application Commands | Yes | Yes |
| User context commands | Application Commands | No | Yes |
| Message context commands | Application Commands | No | Yes |
| Autocomplete | Interaction callbacks | No | Yes |
| Buttons | Message Components | No | Yes |
| String/user/role/channel/mentionable selects | Message Components | No | Yes |
| Modal forms | Modal interactions | No | Yes |
| Ephemeral responses | Interactions | Yes | Yes |
| Deferred and follow-up responses | Interactions/webhooks | No | Yes |
| Embeds | Messages | No | Yes |
| File uploads and attachments | Messages | No | Yes |
| Reactions | Messages | No | Yes |
| Poll creation and results | Polls | No | Yes |
| Webhooks | Webhooks | No | Yes |
| Forum posts and tags | Guild channels | No | Yes |
| Public/private threads | Threads | No | Yes |
| Scheduled events | Guild Scheduled Events | No | Yes |
| Voice connections | Voice Gateway | Yes | Yes |
| Stage channels | Stage Instances | No | Yes |
| Discord AutoMod rules/actions | Auto Moderation | No | Yes |
| Audit-log reads | Audit Log | No | Yes |
| Entitlements/subscriptions | Monetization API | No | Research |

## Gateway events and data

The final event SDK should expose only typed, permission-aware subsets of events to Lua modules.

| Event/data group | Examples | Target modules |
|---|---|---|
| Guild lifecycle | bot added/removed, guild update | platform audit, provisioning |
| Member lifecycle | join, leave, update, ban, unban | welcome, autorole, anti-raid, logs |
| Messages | create, update, delete, bulk delete | automod, XP, starboard, logs |
| Reactions | add/remove | roles, starboard, games |
| Channels/threads | create/update/delete, thread members | logs, tickets, temporary channels |
| Roles | create/update/delete | role restore, anti-nuke, logs |
| Voice | joins, leaves, moves, mute/deaf | voice XP, temporary rooms, analytics |
| Presence | status/activity where available | role sync, analytics with privacy controls |
| Invites | invite create/delete and usage inference | invite tracking, anti-raid |
| Scheduled events | create/update/delete/user add/remove | event automation |
| AutoMod | rule events and action execution | safety logs and case creation |
| Interactions | commands, components, modals, autocomplete | all interactive modules |

## First-party module families

### Moderation and safety

- warnings, notes, cases and evidence;
- kick, ban, soft ban, unban and timeout;
- message purge and bulk actions;
- slow mode and emergency lockdown;
- anti-spam, anti-flood and mention limits;
- invite, link, domain, attachment and word filters;
- phishing and confusable-domain detection;
- anti-raid and account-age verification;
- anti-nuke with trusted-role and recovery controls;
- quarantine and verification workflows;
- native Discord AutoMod synchronization;
- mod logs, appeal workflows and staff analytics.

**0.1:** kick and ban action pipeline with duplicate permission enforcement.

### Roles, onboarding and identity

- welcome/goodbye messages;
- autoroles and delayed roles;
- button/select role menus;
- mutually exclusive and prerequisite roles;
- temporary and expiring roles;
- role restore after rejoin;
- verification gates;
- external account linking;
- nickname templates and synchronization;
- birthday and membership-anniversary automation.

### Community and engagement

- XP from messages and voice with anti-farming;
- levels, rank cards, prestige and achievements;
- role rewards and seasonal leaderboards;
- reputation and thanks;
- economy, inventory, shops and quests;
- polls, suggestions and voting;
- starboard/highlight channels;
- giveaways and rerolls;
- reminders, birthdays and recurring events;
- counting, trivia, mini-games and tournaments;
- looking-for-group and temporary voice rooms.

**0.1:** coin flip, dice roll and text meme examples.

### Music and voice

- URL/search playback and queue;
- join, leave, pause, resume, skip and stop;
- now-playing components;
- loop, shuffle, autoplay, seek and volume;
- DJ roles, vote skip and queue quotas;
- playlists, history and favorites;
- radio and podcast streams;
- metadata adapters for music catalog services;
- audio filters and normalization;
- regional voice workers and queue recovery;
- Stage channel automation and scheduled radio.

**0.1:** Songbird connection and basic queue controls. Runtime deployment needs `ffmpeg` and `yt-dlp`.

### Tickets, forms and support

- button/select ticket panels;
- modal intake forms;
- private channel or private-thread backends;
- assignment, tags, priorities and SLA timers;
- transfer, merge, close, reopen and escalation;
- transcripts and satisfaction surveys;
- canned responses and knowledge-base links;
- external helpdesk adapters.

### Content and notifications

- YouTube, Twitch, Kick and other creator notifications;
- RSS/Atom and podcast feeds;
- Reddit and social feed adapters where APIs permit;
- GitHub releases, commits, issues, pull requests and CI;
- Steam/game-server status;
- calendars and scheduled-event synchronization;
- signed inbound webhooks;
- announcement templates, cross-posting and localization.

### Logging, analytics and operations

- message/member/channel/role/voice logs;
- configuration and moderation audit trails;
- growth, retention and activity dashboards;
- command usage, latency and error metrics;
- scheduled reports and privacy-aware exports;
- Prometheus metrics and OpenTelemetry traces;
- backups, restore tools and incident health pages.

### Developer platform

- versioned Lua SDK;
- event subscriptions;
- typed component/modal builders;
- quota-controlled persistent key-value state;
- durable schedules and jobs;
- domain-allow-listed HTTP adapters;
- module settings schemas and migrations;
- simulator, test harness and developer CLI;
- signed packages, provenance and curated marketplace;
- per-guild version pinning and rollback.

## Dashboard control plane

| Area | 0.1 | Target |
|---|---:|---:|
| Responsive web interface | Yes | Yes |
| Health check | Yes | Yes |
| List installed Lua modules | Yes | Yes |
| Enable/disable modules per guild | Yes | Yes |
| Edit per-module JSON | Yes | Schema-driven forms |
| Atomic Lua reload | Yes | Staged rollout and rollback |
| Owner Bearer token | Yes | Break-glass only |
| Discord OAuth2 | No | Yes |
| Manageable-guild filtering | No | Yes |
| Staff roles/RBAC | No | Yes |
| Configuration audit log | No | Yes |
| Live Gateway status | No | Yes |
| Analytics and moderation cases | No | Yes |
| Billing/entitlements | No | Research |

## Privileged intents

Some functionality requires enabling privileged Gateway intents in the Discord Developer Portal and may require Discord approval at scale:

- `MESSAGE_CONTENT` for arbitrary message-content automod, XP and commands that inspect message text;
- `GUILD_MEMBERS` for complete member lifecycle and member-list workflows;
- `GUILD_PRESENCES` for presence/activity features.

The bot should request only the intents required by enabled product capabilities and document why each is used.

## Platform and policy boundaries

ZuckerBot will not:

- automate normal user accounts or implement self-bot behavior;
- bypass Discord permissions, rate limits, verification or safety controls;
- promise access to events/data Discord does not send to the application;
- expose the bot token to Lua or dashboard browsers;
- allow arbitrary shell, filesystem, database or network access from extensions;
- silently retain private message content beyond configured, disclosed retention;
- autonomously apply irreversible punishment based solely on an AI model score;
- depend on one music extractor/provider so deeply that it cannot be disabled for policy or reliability reasons.

## Primary Discord references

- Application Commands: <https://discord.com/developers/docs/interactions/application-commands>
- Interactions and Message Components: <https://discord.com/developers/docs/interactions/overview>
- Gateway and intents: <https://discord.com/developers/docs/events/gateway>
- Auto Moderation: <https://discord.com/developers/docs/resources/auto-moderation>
- OAuth2: <https://discord.com/developers/docs/topics/oauth2>
- Voice connections: <https://discord.com/developers/docs/topics/voice-connections>

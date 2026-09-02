# Product feature matrix

This document turns the goal “an all-in-one bot with everything” into auditable delivery tracks.

Status:

- **Implemented** — present in the foundation and connected end to end
- **Foundation** — core interface or schema exists, but product workflows are not complete
- **Planned** — accepted scope, not yet implemented
- **Research** — requires policy, cost, quality or scale validation before commitment

The bot should not advertise a feature as complete until its Discord flow, dashboard configuration, persistence, permissions, audit behavior and tests are all present.

## 1. Platform and extensibility

| Capability | Status |
| --- | --- |
| Rust Discord Gateway/HTTP host | Implemented |
| Lua 5.4 module loading | Implemented |
| Command manifests defined in Lua | Implemented |
| Event subscriptions defined in Lua | Implemented |
| Per-execution memory limit | Implemented |
| Per-execution instruction limit | Implemented |
| Allowlisted declarative actions | Implemented |
| Atomic Lua registry reload primitive | Implemented |
| Per-guild module enable/disable | Implemented |
| Per-guild JSON configuration | Implemented |
| PostgreSQL persistence | Implemented |
| In-memory developer fallback | Implemented |
| Discord OAuth2 dashboard login | Implemented |
| CSRF-protected dashboard writes | Implemented |
| Audit events | Implemented |
| JSON Schema form renderer | Planned |
| JSON Schema server-side validation | Planned |
| Live Lua reload from dashboard | Planned |
| Module version pinning per guild | Planned |
| Module dependency graph | Planned |
| Signed first-party modules | Planned |
| Third-party module review pipeline | Planned |
| Module marketplace | Planned |
| Staged rollout and rollback | Planned |
| Feature flags | Planned |
| Localization of commands and dashboard | Planned |
| Multi-process shard supervisor | Planned |
| Distributed session store | Planned |
| Distributed rate-limit coordination | Planned |
| Scheduler workers | Foundation |
| Module key/value persistence API | Foundation |
| Webhook/API trigger system | Planned |
| Public developer SDK and type stubs | Planned |
| Lua language server annotations | Planned |

## 2. Administration

| Capability | Status |
| --- | --- |
| Slash command registration | Implemented |
| Development-guild command mode | Implemented |
| Global command mode | Implemented |
| Role add/remove action | Implemented |
| Actor permission validation | Implemented |
| Bot permission validation for interactions | Implemented |
| Bot and moderator hierarchy validation | Implemented |
| Module configuration audit | Implemented |
| Server setup wizard | Planned |
| Permission diagnostic wizard | Planned |
| Command enable/disable by channel | Planned |
| Command enable/disable by role | Planned |
| Command cooldowns | Planned |
| Per-user and per-role rate limits | Planned |
| Channel templates | Planned |
| Server backup/export | Planned |
| Server configuration import | Planned |
| Clone roles/channels/categories | Planned |
| Temporary roles | Planned |
| Role persistence after rejoin | Planned |
| Self-role buttons and select menus | Planned |
| Reaction roles | Planned |
| Scheduled role assignment | Planned |
| Role synchronization across guilds | Planned |
| Nickname policies | Planned |
| Voice channel rename/limit controls | Planned |
| Temporary voice channels | Planned |
| Server counters in channel names | Planned |

## 3. Moderation and safety

| Capability | Status |
| --- | --- |
| Timeout action | Implemented |
| Kick action | Implemented |
| Ban action | Implemented |
| Message delete action | Implemented |
| Purge up to 100 messages | Implemented |
| Privileged action audit | Implemented |
| Example blocked-word AutoMod module | Implemented |
| Configurable default timeout | Implemented |
| Warnings and moderator notes | Planned |
| Numbered moderation cases | Foundation |
| Case edit and reason update | Planned |
| Temporary bans | Planned |
| Soft bans | Planned |
| Unban workflow | Planned |
| Mass-ban raid response | Planned |
| Quarantine role | Planned |
| Channel lockdown | Planned |
| Category lockdown | Planned |
| Slowmode controls | Planned |
| Anti-spam | Planned |
| Duplicate-message detection | Planned |
| Mention-spam protection | Planned |
| Invite-link filter | Planned |
| Domain allow/block lists | Planned |
| Attachment and file-type rules | Planned |
| Unicode/Zalgo normalization | Planned |
| Caps and emoji flood rules | Planned |
| New-account rules | Planned |
| Join-velocity raid detection | Planned |
| Bot-add protection | Planned |
| Suspicious role-change alerts | Planned |
| Native Discord AutoMod rule synchronization | Planned |
| Verification gate | Planned |
| CAPTCHA provider integration | Research |
| Moderator dashboard case queue | Planned |
| Evidence snapshots | Planned |
| Modmail | Planned |
| Appeals portal | Planned |
| Moderator duty/availability status | Planned |
| Escalation policies | Planned |
| Cross-guild shared ban lists | Research |
| Privacy-aware toxicity classifier | Research |

## 4. Logs and observability

| Capability | Status |
| --- | --- |
| Structured process logs | Implemented |
| Database audit table | Implemented |
| Dashboard change audit | Implemented |
| Moderation action audit | Implemented |
| Message edit/delete logs | Planned |
| Member join/leave logs | Planned |
| Voice join/leave/move logs | Planned |
| Role and permission logs | Planned |
| Channel and guild setting logs | Planned |
| Invite usage tracking | Planned |
| Bot command usage logs | Planned |
| Searchable audit dashboard | Planned |
| Audit retention policies | Planned |
| Privacy redaction policies | Planned |
| Prometheus metrics | Planned |
| OpenTelemetry traces | Planned |
| Error tracking integration | Planned |
| Health and readiness endpoints | Implemented |
| Per-shard status dashboard | Planned |

## 5. Welcome, onboarding and automation

| Capability | Status |
| --- | --- |
| `guild_member_add` Lua event | Implemented |
| Configurable welcome message | Implemented |
| Welcome channel selection | Implemented |
| Event-driven message action | Implemented |
| Goodbye messages | Planned |
| Direct-message onboarding | Planned |
| Autoroles | Planned |
| Rules acceptance | Planned |
| Multi-step onboarding forms | Planned |
| Onboarding role paths | Planned |
| Account-age gates | Planned |
| Invite attribution | Planned |
| Custom command builder | Planned |
| Keyword and regex triggers | Planned |
| Button and select-menu workflows | Planned |
| Modal/form workflows | Planned |
| Scheduled messages | Foundation |
| Recurring schedules | Planned |
| Reminders | Planned |
| Sticky messages | Planned |
| Auto-replies | Planned |
| Cross-channel relays | Planned |
| RSS/Atom feeds | Planned |
| YouTube notifications | Planned |
| Twitch notifications | Planned |
| GitHub release/issue notifications | Planned |
| Reddit notifications | Planned |
| Calendar event reminders | Planned |
| Webhook ingestion | Planned |
| Outgoing webhooks | Planned |
| Conditional workflow branches | Planned |
| Workflow variables and state | Foundation |
| Retry/dead-letter handling | Planned |

## 6. Tickets, support and forms

| Capability | Status |
| --- | --- |
| Ticket panel | Planned |
| Private ticket channels | Planned |
| Ticket categories and routing | Planned |
| Claim/unclaim | Planned |
| Priorities and service levels | Planned |
| Ticket forms | Planned |
| Internal notes | Planned |
| User transcript export | Planned |
| HTML/PDF transcripts | Planned |
| Close reasons | Planned |
| Reopen workflow | Planned |
| Satisfaction surveys | Planned |
| Staff analytics | Planned |
| Modmail-to-ticket bridge | Planned |
| Email integration | Research |
| Knowledge-base suggestions | Research |

## 7. Community engagement

| Capability | Status |
| --- | --- |
| Text memes | Implemented |
| Dice/number roll | Implemented |
| Random choice | Implemented |
| Eight-ball answers | Implemented |
| Polls | Planned |
| Anonymous polls | Planned |
| Giveaways | Planned |
| Reroll and fraud controls | Planned |
| Suggestions with voting | Planned |
| Starboard | Planned |
| Counting channels | Planned |
| Word-chain games | Planned |
| Trivia | Planned |
| Quote database | Planned |
| Birthdays | Planned |
| Server anniversaries | Planned |
| Event sign-ups | Planned |
| Attendance tracking | Planned |
| LFG/party finder | Planned |
| Reputation/thanks | Planned |
| Confessions with moderation queue | Planned |
| Question of the day | Planned |
| Custom collectible cards | Planned |
| Achievements | Planned |

## 8. Levels and economy

| Capability | Status |
| --- | --- |
| Text XP | Planned |
| Voice XP | Planned |
| Anti-farming controls | Planned |
| Configurable XP curves | Planned |
| Role rewards | Planned |
| Rank cards | Planned |
| Leaderboards | Planned |
| Seasonal resets | Planned |
| Import from other bots | Planned |
| Virtual currency | Planned |
| Daily/weekly rewards | Planned |
| Shop and inventory | Planned |
| Tradable items | Planned |
| Gambling-style commands | Excluded pending legal review |
| Work/minigame income | Planned |
| Economy sinks and taxes | Planned |
| Per-guild currency branding | Planned |
| Fraud and alt detection | Planned |
| Economy audit ledger | Planned |

## 9. Music and voice

| Capability | Status |
| --- | --- |
| Join invoking user's voice channel | Implemented |
| Search query playback | Implemented |
| Allowlisted HTTPS URL playback | Implemented |
| Queue | Implemented |
| Pause/resume | Implemented |
| Skip | Implemented |
| Stop and clear queue | Implemented |
| Leave | Implemented |
| URL host allowlist | Implemented |
| Per-guild queue state | Implemented through Songbird |
| Now-playing metadata | Planned |
| Queue display and pagination | Planned |
| Track remove/move/shuffle | Planned |
| Repeat track/queue | Planned |
| Volume control | Planned |
| DJ role and vote-skip | Planned |
| Saved playlists | Planned |
| User favorites and history | Planned |
| 24/7 mode | Planned |
| Radio streams | Planned |
| Audio filters/equalizer | Planned |
| Sponsor-block style metadata integration | Research |
| Multiple source adapters | Planned |
| Dedicated voice nodes | Planned |
| Voice receive/recording | Excluded until consent and privacy design |
| Soundboard management | Planned |
| Text-to-speech | Research |
| Stage channel controls | Planned |
| Karaoke/lyrics display | Research; licensing required |

Music integrations must comply with source-platform terms, copyright rules and Discord policy.

## 10. Utility

| Capability | Status |
| --- | --- |
| Ping/runtime diagnostic | Implemented |
| Bot information | Implemented |
| User information | Planned |
| Avatar/banner lookup | Planned |
| Server information | Planned |
| Role/channel information | Planned |
| Snowflake/timestamp tools | Planned |
| Reminders | Planned |
| Time zones | Planned |
| Weather | Planned |
| Translation | Planned |
| Unit and currency conversion | Planned |
| Calculator | Planned |
| URL preview controls | Planned |
| QR codes | Planned |
| Short links | Excluded unless abuse controls exist |
| File conversion | Research |
| Image manipulation | Planned with resource quotas |
| Meme image generator | Planned |
| Screenshot service | Research |
| Status-page checks | Planned |
| Minecraft/game server status | Planned |
| Steam/game profile lookup | Planned |
| Path of Exile integrations | Planned as optional modules |
| Hero Siege integrations | Planned as optional modules |

## 11. Analytics and dashboard

| Capability | Status |
| --- | --- |
| Discord OAuth2 login | Implemented |
| Manageable-guild filtering | Implemented |
| Module list | Implemented |
| Enable/disable modules | Implemented |
| JSON configuration editor | Implemented |
| Responsive first-party UI | Implemented |
| Module configuration timestamps | Implemented |
| Command analytics | Planned |
| Member growth | Planned |
| Retention/cohort analytics | Planned |
| Message and voice activity | Planned |
| Channel heat maps | Planned |
| Moderation trends | Planned |
| Ticket analytics | Planned |
| Level/economy analytics | Planned |
| Music analytics | Planned |
| Custom dashboards | Planned |
| CSV/JSON export | Planned |
| Scheduled reports | Planned |
| Data retention controls | Planned |
| Consent and privacy center | Planned |
| Multi-user dashboard roles | Planned |
| Organization/multi-guild view | Planned |
| Mobile-optimized administration | Foundation |

## 12. AI-assisted features

AI features are optional modules, not a prerequisite for core moderation.

| Capability | Status |
| --- | --- |
| FAQ assistant grounded in approved documents | Research |
| Support-ticket summarization | Research |
| Moderation case summarization | Research |
| Suggested moderator responses | Research |
| Semantic search across approved knowledge | Research |
| Conversation summaries with consent | Research |
| Translation assistance | Research |
| Spam/scam classifier | Research |
| Image safety classifier | Research |
| Natural-language automation builder | Research |
| Per-guild model/provider choice | Planned if AI track proceeds |
| Cost quotas and hard limits | Required before release |
| Privacy redaction and retention | Required before release |
| Human review for enforcement | Required before release |

No AI model may autonomously ban users without an explicit, reviewable guild policy and an appeal path.

## Delivery order

### Phase 0 — foundation

Current pull request:

- Rust/Lua trust boundary
- commands and first events
- dashboard authentication and module settings
- persistent storage
- initial moderation, role, music, fun and welcome modules
- CI, Docker and documentation

### Phase 1 — production moderation

- cases, warnings, temp actions and full logs
- anti-spam, anti-raid and native AutoMod synchronization
- dashboard permission diagnostics
- test coverage for Discord authorization edge cases
- metrics, backups and operational runbooks

### Phase 2 — automation and community

- workflows, scheduled jobs, role menus and tickets
- levels, reputation, suggestions, polls and giveaways
- external notification adapters
- visual configuration forms

### Phase 3 — advanced voice and analytics

- playlist persistence, DJ controls, voice nodes
- complete analytics and report exports
- multi-guild dashboard roles
- scale testing and shard orchestration

### Phase 4 — ecosystem

- signed modules, review tooling and marketplace
- public SDK and documentation site
- optional privacy-reviewed AI modules

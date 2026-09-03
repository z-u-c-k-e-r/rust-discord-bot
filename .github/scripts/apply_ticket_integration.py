from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    if old not in content:
        raise SystemExit(f"expected source fragment is missing in {path}: {old[:160]!r}")
    file.write_text(content.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/lib.rs",
    "pub mod storage;\npub mod web;\n",
    "pub mod storage;\npub mod tickets;\npub mod web;\n",
)

replace_once(
    "src/state.rs",
    "    scheduler::SchedulerStore,\n    storage::Storage,\n",
    "    scheduler::SchedulerStore,\n    storage::Storage,\n    tickets::TicketStore,\n",
)
replace_once(
    "src/state.rs",
    "    pub scheduler: SchedulerStore,\n    pub storage: Storage,\n",
    "    pub scheduler: SchedulerStore,\n    pub storage: Storage,\n    pub tickets: TicketStore,\n",
)
replace_once(
    "src/state.rs",
    "        let scheduler = SchedulerStore::connect(config.database_url.as_deref()).await?;\n        let user_agent",
    "        let scheduler = SchedulerStore::connect(config.database_url.as_deref()).await?;\n        let tickets = TicketStore::connect(config.database_url.as_deref()).await?;\n        let user_agent",
)
replace_once(
    "src/state.rs",
    "            scheduler,\n            storage,\n            http_client,\n",
    "            scheduler,\n            storage,\n            tickets,\n            http_client,\n",
)

replace_once(
    "src/lua/mod.rs",
    "mod scheduler;\n",
    "mod scheduler;\nmod tickets;\n",
)
replace_once(
    "src/lua/mod.rs",
    "pub use scheduler::{SchedulerOperation, SchedulerScope};\n",
    "pub use scheduler::{SchedulerOperation, SchedulerScope};\npub use tickets::{\n    TicketClosePolicy, TicketOpenPolicy, TicketOperation, TicketPriority, TicketScope,\n};\n",
)

replace_once(
    "src/lua/model.rs",
    "use super::{progression::ProgressionOperation, scheduler::SchedulerOperation};\n",
    "use super::{\n    progression::ProgressionOperation, scheduler::SchedulerOperation, tickets::TicketOperation,\n};\n",
)
replace_once(
    "src/lua/model.rs",
    "    Scheduler {\n        operation: SchedulerOperation,\n    },\n    Audit {\n",
    "    Scheduler {\n        operation: SchedulerOperation,\n    },\n    Ticket {\n        operation: TicketOperation,\n    },\n    Audit {\n",
)
replace_once(
    "src/lua/model.rs",
    "            Self::Scheduler { operation } => operation.validate(),\n            Self::Audit { event, .. } => {\n",
    "            Self::Scheduler { operation } => operation.validate(),\n            Self::Ticket { operation } => operation.validate(),\n            Self::Audit { event, .. } => {\n",
)

replace_once(
    "src/discord/mod.rs",
    "mod scheduler;\n",
    "mod scheduler;\nmod tickets;\n",
)

replace_once(
    "src/discord/executor.rs",
    "use super::{music, progression, scheduler};\n",
    "use super::{music, progression, scheduler, tickets};\n",
)
replace_once(
    "src/discord/executor.rs",
    "        LuaAction::Audit { event, data } => {\n",
    "        LuaAction::Ticket { operation } => {\n            tickets::execute(\n                ctx,\n                state,\n                module_id,\n                tickets::TicketExecutionContext::new(\n                    origin.guild_id,\n                    origin.channel_id,\n                    origin.actor_id,\n                    origin.actor_permissions,\n                    origin.app_permissions,\n                    origin.enforce_actor_permissions,\n                ),\n                operation,\n            )\n            .await\n        }\n        LuaAction::Audit { event, data } => {\n",
)

replace_once(
    "src/discord/tickets.rs",
    "        TicketOperation::Info => {\n            let ticket = require_current_ticket(state, guild_id, channel_id).await?;\n            ensure_ticket_access(\n                ctx,\n                state,\n                &ticket,\n                guild_id,\n                actor_id,\n                execution.actor_permissions,\n                &[],\n            )\n            .await?;\n            let participants = state.tickets.list_participants(&ticket.id).await?;\n",
    "        TicketOperation::Info => {\n            // Reaching this command already requires Discord View Channel access. The ticket\n            // operation carries no policy snapshot, so re-checking an empty role set would\n            // incorrectly reject support-role members who can legitimately see the channel.\n            let ticket = require_current_ticket(state, guild_id, channel_id).await?;\n            let participants = state.tickets.list_participants(&ticket.id).await?;\n",
)

replace_once(
    "src/discord/tickets.rs",
    "    let log_channel = resolve_optional_log_channel(ctx, guild_id, policy.log_channel_id.as_deref())\n        .await?;\n    let transcript = if policy.generate_transcript {\n",
    "    let log_channel = resolve_optional_log_channel(ctx, guild_id, policy.log_channel_id.as_deref())\n        .await?;\n    let archive_category = match policy.archive_category_id.as_deref() {\n        Some(category_id) => Some(ensure_category(ctx, guild_id, category_id).await?),\n        None => None,\n    };\n    let transcript = if policy.generate_transcript {\n",
)
replace_once(
    "src/discord/tickets.rs",
    "    let mut edit = EditChannel::new().name(&closed_name);\n    if let Some(category_id) = policy.archive_category_id.as_deref() {\n        edit = edit.category(ensure_category(ctx, guild_id, category_id).await?);\n    }\n",
    "    let mut edit = EditChannel::new().name(&closed_name);\n    if let Some(archive_category) = archive_category {\n        edit = edit.category(archive_category);\n    }\n",
)

replace_once(
    "src/discord/tickets.rs",
    "    if member.user.bot {\n        return Err(anyhow!(\"bot nie może zostać uczestnikiem ticketu\"));\n    }\n\n    if add {\n",
    "    if member.user.bot {\n        return Err(anyhow!(\"bot nie może zostać uczestnikiem ticketu\"));\n    }\n\n    // Resolve the domain state before changing Discord overwrites. Otherwise an idempotent\n    // add could revoke an existing participant, while removing an unknown user could grant\n    // that user access during rollback.\n    if ticket.creator_user_id == user_id {\n        return Ok(Some(format_participant_outcome(\n            ParticipantMutationOutcome::CreatorProtected,\n        )));\n    }\n    let participants = state.tickets.list_participants(&ticket.id).await?;\n    let is_participant = participants\n        .iter()\n        .any(|participant| participant.user_id == user_id);\n    if add && is_participant {\n        return Ok(Some(format_participant_outcome(\n            ParticipantMutationOutcome::AlreadyPresent,\n        )));\n    }\n    if !add && !is_participant {\n        return Ok(Some(format_participant_outcome(\n            ParticipantMutationOutcome::NotFound,\n        )));\n    }\n\n    if add {\n",
)

replace_once(
    "src/tickets/memory.rs",
    "    pub fn rename_ticket(\n        &self,\n        ticket_id: &str,\n        actor_user_id: &str,\n        channel_name: &str,\n    ) -> TicketMutationOutcome {\n        self.update_open_ticket(\n            ticket_id,\n            actor_user_id,\n            TicketMutation::Rename,\n            json!({ \"channel_name\": channel_name }),\n            |ticket| ticket.rename(channel_name.to_owned(), Utc::now()),\n        )\n    }\n",
    "    pub fn rename_ticket(\n        &self,\n        ticket_id: &str,\n        actor_user_id: &str,\n        channel_name: &str,\n    ) -> TicketMutationOutcome {\n        let mut state = self.lock();\n        let updated = {\n            let Some(ticket) = state.tickets.get_mut(ticket_id) else {\n                return TicketMutationOutcome::NotFound;\n            };\n            if !ticket.is_open() && ticket.status != STATUS_CLOSED {\n                return TicketMutationOutcome::InvalidState {\n                    current_status: ticket.status.clone(),\n                };\n            }\n            ticket.rename(channel_name.to_owned(), Utc::now());\n            ticket.clone()\n        };\n        push_event(\n            &mut state,\n            &updated.id,\n            &updated.guild_id,\n            Some(actor_user_id),\n            TicketMutation::Rename.event_name(),\n            json!({ \"channel_name\": channel_name }),\n        );\n        TicketMutationOutcome::Updated(Box::new(updated))\n    }\n",
)
replace_once(
    "src/tickets/memory.rs",
    "    state.next_event_id = state.next_event_id.saturating_add(1);\n    state.events.push(TicketEvent {\n        id: state.next_event_id,\n",
    "    state.next_event_id = state.next_event_id.saturating_add(1);\n    let event_id = state.next_event_id;\n    state.events.push(TicketEvent {\n        id: event_id,\n",
)

replace_once(
    "src/tickets/postgres.rs",
    "            TicketMutation::Rename.event_name(),\n            json!({ \"channel_name\": channel_name }),\n            |ticket| {\n                if !ticket.is_open() {\n",
    "            TicketMutation::Rename.event_name(),\n            json!({ \"channel_name\": channel_name }),\n            |ticket| {\n                if !ticket.is_open() && ticket.status != STATUS_CLOSED {\n",
)

replace_once(
    ".github/workflows/ci.yml",
    "      - feat/persistent-scheduler-reminders\n",
    "      - feat/persistent-scheduler-reminders\n      - feat/ticket-support-workflows\n",
)

replace_once(
    "README.md",
    "- validated action API for replies, messages, moderation, roles, purge, music, progression, persistent scheduling and audit events\n",
    "- validated action API for replies, messages, moderation, roles, purge, music, progression, persistent scheduling, private tickets and audit events\n",
)
replace_once(
    "README.md",
    "- audit records for privileged actions, scheduler transitions and dashboard changes\n- lease-based PostgreSQL scheduler with retries, pause/resume and an in-memory test fallback\n",
    "- audit records for privileged actions, scheduler transitions, ticket lifecycles and dashboard changes\n- lease-based PostgreSQL scheduler with retries, pause/resume and an in-memory test fallback\n- transactional private-ticket workflows with queues, claim, participants, archive and transcripts\n",
)
replace_once(
    "README.md",
    "| `scheduler.lua` | persistent reminders and recurring server messages |\n",
    "| `scheduler.lua` | persistent reminders and recurring server messages |\n| `tickets.lua` | private support tickets, queues, claim, archive and transcripts |\n",
)
replace_once(
    "README.md",
    "  storage/       PostgreSQL and in-memory stores\n  web/           OAuth2 dashboard and API\n",
    "  storage/       guild configuration, audit and module data stores\n  scheduler/     persistent jobs, leasing, retries and recurrence\n  tickets/       persistent support cases, participants and transcripts\n  web/           OAuth2 dashboard and API\n",
)
replace_once(
    "README.md",
    "Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for trust boundaries and request flows.\n",
    "Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for trust boundaries and request flows. The scheduler contract is documented in [`docs/SCHEDULER.md`](docs/SCHEDULER.md), and the private support workflow in [`docs/TICKETS.md`](docs/TICKETS.md).\n",
)

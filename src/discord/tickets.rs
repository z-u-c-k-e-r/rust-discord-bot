use std::collections::HashSet;

use anyhow::{Context as _, Result, anyhow};
use serenity::{
    all::{
        Channel, ChannelId, ChannelType, GuildId, Message, MessageId, PermissionOverwrite,
        PermissionOverwriteType, Permissions, RoleId, UserId,
    },
    builder::{
        CreateAllowedMentions, CreateAttachment, CreateChannel, CreateMessage, EditChannel,
        GetMessages,
    },
    client::Context,
};

use crate::{
    AppState,
    lua::{
        TicketClosePolicy, TicketOpenPolicy, TicketOperation, TicketPriority, TicketScope,
    },
    tickets::{
        NewTicketReservation, NewTicketTranscript, ParticipantMutationOutcome,
        ReserveTicketOutcome, STATUS_CLAIMED, STATUS_CLOSED, STATUS_FAILED, STATUS_OPEN,
        STATUS_PROVISIONING, TicketMutationOutcome, TicketRecord,
    },
};

const MAX_TRANSCRIPT_BYTES: usize = 7 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct TicketExecutionContext {
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    app_permissions: Permissions,
    command_context: bool,
}

impl TicketExecutionContext {
    pub(super) const fn new(
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        actor_id: Option<UserId>,
        actor_permissions: Permissions,
        app_permissions: Permissions,
        command_context: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            actor_id,
            actor_permissions,
            app_permissions,
            command_context,
        }
    }
}

pub async fn execute(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    execution: TicketExecutionContext,
    operation: &TicketOperation,
) -> Result<Option<String>> {
    if module_id != "tickets" {
        return Err(anyhow!(
            "operacje ticketów są zarezerwowane dla zaufanego modułu tickets"
        ));
    }
    if !execution.command_context {
        return Err(anyhow!(
            "ticketami można zarządzać wyłącznie przez komendy użytkowników"
        ));
    }

    let guild_id = execution
        .guild_id
        .ok_or_else(|| anyhow!("system ticketów działa wyłącznie na serwerze"))?;
    let channel_id = execution
        .channel_id
        .ok_or_else(|| anyhow!("operacja ticketu wymaga kanału"))?;
    let actor_id = execution
        .actor_id
        .ok_or_else(|| anyhow!("operacja ticketu wymaga użytkownika"))?;
    let guild_key = guild_id.get().to_string();
    let actor_key = actor_id.get().to_string();

    match operation {
        TicketOperation::Open {
            subject,
            description,
            queue,
            policy,
        } => {
            open_ticket(
                ctx,
                state,
                guild_id,
                actor_id,
                execution.app_permissions,
                subject,
                description,
                queue,
                policy,
            )
            .await
        }
        TicketOperation::List {
            scope,
            limit,
            support_role_ids,
        } => {
            let include_all = matches!(scope, TicketScope::All);
            if include_all
                && !actor_is_staff(
                    ctx,
                    guild_id,
                    actor_id,
                    execution.actor_permissions,
                    support_role_ids,
                )
                .await?
            {
                return Err(anyhow!(
                    "lista wszystkich ticketów wymaga roli wsparcia lub uprawnienia Zarządzanie kanałami"
                ));
            }
            let tickets = state
                .tickets
                .list_tickets(&guild_key, &actor_key, include_all, *limit)
                .await?;
            Ok(Some(format_ticket_list(&tickets, include_all)))
        }
        TicketOperation::Info => {
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            ensure_ticket_access(
                ctx,
                state,
                &ticket,
                guild_id,
                actor_id,
                execution.actor_permissions,
                &[],
            )
            .await?;
            let participants = state.tickets.list_participants(&ticket.id).await?;
            let events = state.tickets.list_events(&ticket.id, 10).await?;
            Ok(Some(format_ticket_info(&ticket, &participants, &events)))
        }
        TicketOperation::Claim { support_role_ids } => {
            ensure_bot_permissions(execution.app_permissions, Permissions::SEND_MESSAGES)?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            ensure_staff(
                ctx,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            match state.tickets.claim_ticket(&ticket.id, &actor_key).await? {
                TicketMutationOutcome::Updated(updated) => {
                    send_system_message(
                        ctx,
                        channel_id,
                        &format!("🧑‍💻 Ticket przejął <@{}>.", actor_id.get()),
                        Some(actor_id),
                    )
                    .await?;
                    audit_ticket(state, &updated, Some(&actor_key), "ticket_claimed").await?;
                    Ok(Some(format!(
                        "✅ Przejęto ticket **#{}**.",
                        updated.display_number()
                    )))
                }
                outcome => Ok(Some(format_mutation_outcome(outcome))),
            }
        }
        TicketOperation::Unclaim { support_role_ids } => {
            ensure_bot_permissions(execution.app_permissions, Permissions::SEND_MESSAGES)?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            let staff = actor_is_staff(
                ctx,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            let allow_any = staff && ticket.claimed_by_user_id.as_deref() != Some(&actor_key);
            match state
                .tickets
                .unclaim_ticket(&ticket.id, &actor_key, allow_any)
                .await?
            {
                TicketMutationOutcome::Updated(updated) => {
                    send_system_message(
                        ctx,
                        channel_id,
                        &format!("↩️ <@{}> zwolnił ticket.", actor_id.get()),
                        Some(actor_id),
                    )
                    .await?;
                    audit_ticket(state, &updated, Some(&actor_key), "ticket_unclaimed").await?;
                    Ok(Some(format!(
                        "✅ Zwolniono ticket **#{}**.",
                        updated.display_number()
                    )))
                }
                outcome => Ok(Some(format_mutation_outcome(outcome))),
            }
        }
        TicketOperation::Close { reason, policy } => {
            close_ticket(
                ctx,
                state,
                guild_id,
                channel_id,
                actor_id,
                execution.actor_permissions,
                execution.app_permissions,
                reason.as_deref(),
                policy,
            )
            .await
        }
        TicketOperation::Reopen {
            open_category_id,
            support_role_ids,
        } => {
            ensure_bot_permissions(
                execution.app_permissions,
                Permissions::MANAGE_CHANNELS | Permissions::SEND_MESSAGES,
            )?;
            ensure_staff(
                ctx,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            if ticket.status != STATUS_CLOSED {
                return Ok(Some(format!(
                    "Ticket ma stan **{}**, więc nie można go ponownie otworzyć.",
                    status_label(&ticket.status)
                )));
            }
            let open_category = ensure_category(ctx, guild_id, open_category_id).await?;
            let participants = state.tickets.list_participants(&ticket.id).await?;
            restore_member_access(ctx, channel_id, UserId::new(parse_snowflake(&ticket.creator_user_id)?))
                .await?;
            for participant in &participants {
                restore_member_access(
                    ctx,
                    channel_id,
                    UserId::new(parse_snowflake(&participant.user_id)?),
                )
                .await?;
            }
            let reopened_name = ticket
                .channel_name
                .as_deref()
                .map(remove_closed_prefix)
                .unwrap_or_else(|| format!("ticket-{}", ticket.display_number()));
            channel_id
                .edit(
                    &ctx.http,
                    EditChannel::new()
                        .name(&reopened_name)
                        .category(open_category),
                )
                .await?;

            match state.tickets.reopen_ticket(&ticket.id, &actor_key).await? {
                TicketMutationOutcome::Updated(updated) => {
                    if updated.channel_name.as_deref() != Some(&reopened_name) {
                        let _ = state
                            .tickets
                            .rename_ticket(&updated.id, &actor_key, &reopened_name)
                            .await;
                    }
                    send_system_message(
                        ctx,
                        channel_id,
                        &format!("🔓 Ticket ponownie otworzył <@{}>.", actor_id.get()),
                        Some(actor_id),
                    )
                    .await?;
                    audit_ticket(state, &updated, Some(&actor_key), "ticket_reopened").await?;
                    Ok(Some(format!(
                        "✅ Ponownie otwarto ticket **#{}**.",
                        updated.display_number()
                    )))
                }
                outcome => Ok(Some(format_mutation_outcome(outcome))),
            }
        }
        TicketOperation::AddMember {
            user_id,
            support_role_ids,
            creator_can_manage_participants,
        } => {
            manage_participant(
                ctx,
                state,
                guild_id,
                channel_id,
                actor_id,
                execution.actor_permissions,
                execution.app_permissions,
                user_id,
                support_role_ids,
                *creator_can_manage_participants,
                true,
            )
            .await
        }
        TicketOperation::RemoveMember {
            user_id,
            support_role_ids,
            creator_can_manage_participants,
        } => {
            manage_participant(
                ctx,
                state,
                guild_id,
                channel_id,
                actor_id,
                execution.actor_permissions,
                execution.app_permissions,
                user_id,
                support_role_ids,
                *creator_can_manage_participants,
                false,
            )
            .await
        }
        TicketOperation::Rename {
            name,
            support_role_ids,
            creator_can_rename,
        } => {
            ensure_bot_permissions(execution.app_permissions, Permissions::MANAGE_CHANNELS)?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            let staff = actor_is_staff(
                ctx,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            if !staff && !(*creator_can_rename && ticket.creator_user_id == actor_key) {
                return Err(anyhow!(
                    "zmiana nazwy wymaga roli wsparcia albo uprawnienia właściciela ticketu"
                ));
            }
            let sanitized_name = sanitize_channel_name(name);
            let previous_name = ticket.channel_name.clone();
            channel_id
                .edit(&ctx.http, EditChannel::new().name(&sanitized_name))
                .await?;
            match state
                .tickets
                .rename_ticket(&ticket.id, &actor_key, &sanitized_name)
                .await?
            {
                TicketMutationOutcome::Updated(updated) => {
                    audit_ticket(state, &updated, Some(&actor_key), "ticket_renamed").await?;
                    Ok(Some(format!(
                        "✅ Nazwa ticketu została zmieniona na `{sanitized_name}`."
                    )))
                }
                outcome => {
                    if let Some(previous_name) = previous_name {
                        let _ = channel_id
                            .edit(&ctx.http, EditChannel::new().name(previous_name))
                            .await;
                    }
                    Ok(Some(format_mutation_outcome(outcome)))
                }
            }
        }
        TicketOperation::SetPriority {
            priority,
            support_role_ids,
        } => {
            ensure_staff(
                ctx,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            match state
                .tickets
                .set_priority(&ticket.id, &actor_key, priority.as_str())
                .await?
            {
                TicketMutationOutcome::Updated(updated) => {
                    send_system_message(
                        ctx,
                        channel_id,
                        &format!(
                            "🚦 Priorytet ticketu ustawiono na **{}**.",
                            priority_label(*priority)
                        ),
                        None,
                    )
                    .await?;
                    audit_ticket(
                        state,
                        &updated,
                        Some(&actor_key),
                        "ticket_priority_changed",
                    )
                    .await?;
                    Ok(Some(format!(
                        "✅ Priorytet ticketu **#{}**: **{}**.",
                        updated.display_number(),
                        priority_label(*priority)
                    )))
                }
                outcome => Ok(Some(format_mutation_outcome(outcome))),
            }
        }
        TicketOperation::Transcript {
            support_role_ids,
            log_channel_id,
            max_messages,
        } => {
            ensure_bot_permissions(
                execution.app_permissions,
                Permissions::READ_MESSAGE_HISTORY | Permissions::ATTACH_FILES,
            )?;
            let ticket = require_current_ticket(state, guild_id, channel_id).await?;
            ensure_ticket_access(
                ctx,
                state,
                &ticket,
                guild_id,
                actor_id,
                execution.actor_permissions,
                support_role_ids,
            )
            .await?;
            let log_channel = resolve_optional_log_channel(ctx, guild_id, log_channel_id.as_deref())
                .await?;
            let transcript = create_and_store_transcript(
                ctx,
                state,
                &ticket,
                actor_id,
                *max_messages,
            )
            .await?;
            send_transcript(ctx, channel_id, &ticket, &transcript).await?;
            if let Some(log_channel) = log_channel
                && log_channel != channel_id
            {
                send_transcript(ctx, log_channel, &ticket, &transcript).await?;
            }
            audit_ticket(
                state,
                &ticket,
                Some(&actor_key),
                "ticket_transcript_created",
            )
            .await?;
            Ok(Some(format!(
                "✅ Utworzono transkrypcję `{}` zawierającą **{}** wiadomości.",
                transcript.id, transcript.message_count
            )))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn open_ticket(
    ctx: &Context,
    state: &AppState,
    guild_id: GuildId,
    actor_id: UserId,
    app_permissions: Permissions,
    subject: &str,
    description: &str,
    queue: &str,
    policy: &TicketOpenPolicy,
) -> Result<Option<String>> {
    ensure_bot_permissions(
        app_permissions,
        Permissions::MANAGE_CHANNELS
            | Permissions::VIEW_CHANNEL
            | Permissions::SEND_MESSAGES
            | Permissions::READ_MESSAGE_HISTORY,
    )?;
    let open_category = ensure_category(ctx, guild_id, &policy.open_category_id).await?;
    if let Some(category_id) = policy.archive_category_id.as_deref() {
        ensure_category(ctx, guild_id, category_id).await?;
    }
    validate_support_roles(ctx, guild_id, &policy.support_role_ids).await?;
    let log_channel = resolve_optional_log_channel(ctx, guild_id, policy.log_channel_id.as_deref())
        .await?;

    let guild_key = guild_id.get().to_string();
    let actor_key = actor_id.get().to_string();
    let reservation = NewTicketReservation {
        guild_id: guild_key.clone(),
        creator_user_id: actor_key.clone(),
        subject: subject.trim().to_owned(),
        description: description.trim().to_owned(),
        queue: queue.trim().to_owned(),
    };
    let reserved = match state
        .tickets
        .reserve_ticket(reservation, policy.max_open_per_user)
        .await?
    {
        ReserveTicketOutcome::Reserved(ticket) => ticket,
        ReserveTicketOutcome::LimitReached {
            limit,
            active_count,
        } => {
            return Ok(Some(format!(
                "Masz już **{active_count}** aktywnych ticketów. Limit na użytkownika wynosi **{limit}**."
            )));
        }
    };

    let channel_name = build_initial_channel_name(
        &policy.channel_name_prefix,
        reserved.number,
        &reserved.subject,
    );
    let bot_id = ctx.cache.current_user().id;
    let permissions = build_open_permissions(
        guild_id,
        actor_id,
        bot_id,
        &policy.support_role_ids,
    )?;
    let topic = format!(
        "ZuckerBot ticket {} | owner {} | queue {}",
        reserved.id, reserved.creator_user_id, reserved.queue
    );
    let created_channel = match guild_id
        .create_channel(
            &ctx.http,
            CreateChannel::new(&channel_name)
                .kind(ChannelType::Text)
                .category(open_category)
                .topic(topic)
                .permissions(permissions)
                .audit_log_reason("ZuckerBot ticket creation"),
        )
        .await
    {
        Ok(channel) => channel,
        Err(error) => {
            let _ = state
                .tickets
                .fail_provisioning(&reserved.id, &format!("Discord channel creation failed: {error}"))
                .await;
            return Err(error.into());
        }
    };

    let activated = match state
        .tickets
        .activate_ticket(
            &reserved.id,
            &created_channel.id.get().to_string(),
            &channel_name,
        )
        .await?
    {
        TicketMutationOutcome::Updated(ticket) => ticket,
        outcome => {
            let _ = created_channel.id.delete(&ctx.http).await;
            let _ = state
                .tickets
                .fail_provisioning(
                    &reserved.id,
                    &format!("ticket activation failed: {}", format_mutation_outcome(outcome)),
                )
                .await;
            return Err(anyhow!("nie udało się aktywować rekordu ticketu"));
        }
    };

    let welcome = format_welcome_message(&activated, policy.welcome_message.as_deref());
    if let Err(error) = send_system_message(
        ctx,
        created_channel.id,
        &welcome,
        Some(actor_id),
    )
    .await
    {
        tracing::warn!(ticket_id = %activated.id, ?error, "cannot send ticket welcome message");
    }
    audit_ticket(state, &activated, Some(&actor_key), "ticket_opened").await?;
    if let Some(log_channel) = log_channel {
        let log_message = format!(
            "🎫 Otwarto ticket **#{}** w <#{}> przez <@{}>. Kolejka: `{}`. Temat: **{}**",
            activated.display_number(),
            created_channel.id.get(),
            actor_id.get(),
            activated.queue,
            activated.subject,
        );
        if let Err(error) = send_system_message(ctx, log_channel, &log_message, Some(actor_id)).await {
            tracing::warn!(ticket_id = %activated.id, ?error, "cannot send ticket log message");
        }
    }

    Ok(Some(format!(
        "✅ Utworzono ticket **#{}**: <#{}>",
        activated.display_number(),
        created_channel.id.get()
    )))
}

#[allow(clippy::too_many_arguments)]
async fn close_ticket(
    ctx: &Context,
    state: &AppState,
    guild_id: GuildId,
    channel_id: ChannelId,
    actor_id: UserId,
    actor_permissions: Permissions,
    app_permissions: Permissions,
    reason: Option<&str>,
    policy: &TicketClosePolicy,
) -> Result<Option<String>> {
    ensure_bot_permissions(
        app_permissions,
        Permissions::MANAGE_CHANNELS
            | Permissions::SEND_MESSAGES
            | Permissions::READ_MESSAGE_HISTORY,
    )?;
    let ticket = require_current_ticket(state, guild_id, channel_id).await?;
    let actor_key = actor_id.get().to_string();
    let staff = actor_is_staff(
        ctx,
        guild_id,
        actor_id,
        actor_permissions,
        &policy.support_role_ids,
    )
    .await?;
    if !staff && !(policy.creator_can_close && ticket.creator_user_id == actor_key) {
        return Err(anyhow!(
            "zamknięcie ticketu wymaga roli wsparcia albo uprawnienia właściciela ticketu"
        ));
    }
    if !ticket.is_open() {
        return Ok(Some(format!(
            "Ticket ma stan **{}** i nie może zostać zamknięty.",
            status_label(&ticket.status)
        )));
    }

    let log_channel = resolve_optional_log_channel(ctx, guild_id, policy.log_channel_id.as_deref())
        .await?;
    let transcript = if policy.generate_transcript {
        let transcript = create_and_store_transcript(
            ctx,
            state,
            &ticket,
            actor_id,
            policy.transcript_max_messages,
        )
        .await?;
        send_transcript(ctx, channel_id, &ticket, &transcript).await?;
        if let Some(log_channel) = log_channel
            && log_channel != channel_id
        {
            if let Err(error) = send_transcript(ctx, log_channel, &ticket, &transcript).await {
                tracing::warn!(ticket_id = %ticket.id, ?error, "cannot send transcript to log channel");
            }
        }
        Some(transcript)
    } else {
        None
    };

    let participants = state.tickets.list_participants(&ticket.id).await?;
    set_member_read_only(
        ctx,
        channel_id,
        UserId::new(parse_snowflake(&ticket.creator_user_id)?),
    )
    .await?;
    for participant in &participants {
        set_member_read_only(
            ctx,
            channel_id,
            UserId::new(parse_snowflake(&participant.user_id)?),
        )
        .await?;
    }

    let closed_name = build_closed_channel_name(&ticket);
    let mut edit = EditChannel::new().name(&closed_name);
    if let Some(category_id) = policy.archive_category_id.as_deref() {
        edit = edit.category(ensure_category(ctx, guild_id, category_id).await?);
    }
    channel_id.edit(&ctx.http, edit).await?;

    match state
        .tickets
        .close_ticket(&ticket.id, &actor_key, reason)
        .await?
    {
        TicketMutationOutcome::Updated(updated) => {
            if updated.channel_name.as_deref() != Some(&closed_name) {
                let _ = state
                    .tickets
                    .rename_ticket(&updated.id, &actor_key, &closed_name)
                    .await;
            }
            let closed_message = format!(
                "🔒 Ticket zamknął <@{}>. Powód: {}",
                actor_id.get(),
                reason.unwrap_or("nie podano")
            );
            send_system_message(ctx, channel_id, &closed_message, Some(actor_id)).await?;
            audit_ticket(state, &updated, Some(&actor_key), "ticket_closed").await?;
            if let Some(log_channel) = log_channel {
                let log_message = format!(
                    "🔒 Zamknięto ticket **#{}** przez <@{}>. Powód: {}{}",
                    updated.display_number(),
                    actor_id.get(),
                    reason.unwrap_or("nie podano"),
                    transcript
                        .as_ref()
                        .map(|value| format!(". Transkrypcja: `{}`", value.id))
                        .unwrap_or_default(),
                );
                if let Err(error) =
                    send_system_message(ctx, log_channel, &log_message, Some(actor_id)).await
                {
                    tracing::warn!(ticket_id = %updated.id, ?error, "cannot send close log");
                }
            }
            Ok(Some(format!(
                "✅ Zamknięto ticket **#{}**.",
                updated.display_number()
            )))
        }
        outcome => Ok(Some(format_mutation_outcome(outcome))),
    }
}

#[allow(clippy::too_many_arguments)]
async fn manage_participant(
    ctx: &Context,
    state: &AppState,
    guild_id: GuildId,
    channel_id: ChannelId,
    actor_id: UserId,
    actor_permissions: Permissions,
    app_permissions: Permissions,
    user_id: &str,
    support_role_ids: &[String],
    creator_can_manage: bool,
    add: bool,
) -> Result<Option<String>> {
    ensure_bot_permissions(app_permissions, Permissions::MANAGE_CHANNELS)?;
    let ticket = require_current_ticket(state, guild_id, channel_id).await?;
    let actor_key = actor_id.get().to_string();
    let staff = actor_is_staff(
        ctx,
        guild_id,
        actor_id,
        actor_permissions,
        support_role_ids,
    )
    .await?;
    if !staff && !(creator_can_manage && ticket.creator_user_id == actor_key) {
        return Err(anyhow!(
            "zarządzanie uczestnikami wymaga roli wsparcia albo uprawnienia właściciela ticketu"
        ));
    }

    let target_id = UserId::new(parse_snowflake(user_id)?);
    let member = guild_id
        .member(&ctx.http, target_id)
        .await
        .context("użytkownik nie należy do tego serwera")?;
    if member.user.bot {
        return Err(anyhow!("bot nie może zostać uczestnikiem ticketu"));
    }

    if add {
        restore_member_access(ctx, channel_id, target_id).await?;
        match state
            .tickets
            .add_participant(&ticket.id, user_id, &actor_key)
            .await?
        {
            ParticipantMutationOutcome::Added(_) => {
                send_system_message(
                    ctx,
                    channel_id,
                    &format!("➕ <@{}> został dodany do ticketu.", target_id.get()),
                    Some(target_id),
                )
                .await?;
                audit_ticket(
                    state,
                    &ticket,
                    Some(&actor_key),
                    "ticket_participant_added",
                )
                .await?;
                Ok(Some(format!(
                    "✅ Dodano <@{}> do ticketu.",
                    target_id.get()
                )))
            }
            outcome => {
                let _ = channel_id
                    .delete_permission(&ctx.http, PermissionOverwriteType::Member(target_id))
                    .await;
                Ok(Some(format_participant_outcome(outcome)))
            }
        }
    } else {
        channel_id
            .delete_permission(&ctx.http, PermissionOverwriteType::Member(target_id))
            .await?;
        match state
            .tickets
            .remove_participant(&ticket.id, user_id, &actor_key)
            .await?
        {
            ParticipantMutationOutcome::Removed => {
                send_system_message(
                    ctx,
                    channel_id,
                    &format!("➖ <@{}> został usunięty z ticketu.", target_id.get()),
                    Some(target_id),
                )
                .await?;
                audit_ticket(
                    state,
                    &ticket,
                    Some(&actor_key),
                    "ticket_participant_removed",
                )
                .await?;
                Ok(Some(format!(
                    "✅ Usunięto <@{}> z ticketu.",
                    target_id.get()
                )))
            }
            outcome => {
                let _ = restore_member_access(ctx, channel_id, target_id).await;
                Ok(Some(format_participant_outcome(outcome)))
            }
        }
    }
}

async fn require_current_ticket(
    state: &AppState,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<TicketRecord> {
    state
        .tickets
        .get_by_channel(
            &guild_id.get().to_string(),
            &channel_id.get().to_string(),
        )
        .await?
        .ok_or_else(|| anyhow!("ten kanał nie jest zarejestrowanym ticketem"))
}

async fn ensure_ticket_access(
    ctx: &Context,
    state: &AppState,
    ticket: &TicketRecord,
    guild_id: GuildId,
    actor_id: UserId,
    actor_permissions: Permissions,
    support_role_ids: &[String],
) -> Result<()> {
    if ticket.creator_user_id == actor_id.get().to_string()
        || actor_is_staff(
            ctx,
            guild_id,
            actor_id,
            actor_permissions,
            support_role_ids,
        )
        .await?
    {
        return Ok(());
    }
    let actor_key = actor_id.get().to_string();
    if state
        .tickets
        .list_participants(&ticket.id)
        .await?
        .iter()
        .any(|participant| participant.user_id == actor_key)
    {
        return Ok(());
    }
    Err(anyhow!("nie masz dostępu do tego ticketu"))
}

async fn ensure_staff(
    ctx: &Context,
    guild_id: GuildId,
    actor_id: UserId,
    actor_permissions: Permissions,
    support_role_ids: &[String],
) -> Result<()> {
    if actor_is_staff(
        ctx,
        guild_id,
        actor_id,
        actor_permissions,
        support_role_ids,
    )
    .await?
    {
        Ok(())
    } else {
        Err(anyhow!(
            "ta operacja wymaga roli wsparcia lub uprawnienia Zarządzanie kanałami"
        ))
    }
}

async fn actor_is_staff(
    ctx: &Context,
    guild_id: GuildId,
    actor_id: UserId,
    actor_permissions: Permissions,
    support_role_ids: &[String],
) -> Result<bool> {
    if actor_permissions.contains(Permissions::ADMINISTRATOR)
        || actor_permissions.contains(Permissions::MANAGE_CHANNELS)
        || actor_permissions.contains(Permissions::MANAGE_GUILD)
    {
        return Ok(true);
    }
    if support_role_ids.is_empty() {
        return Ok(false);
    }

    let expected = support_role_ids
        .iter()
        .map(|value| parse_snowflake(value).map(RoleId::new))
        .collect::<Result<HashSet<_>>>()?;
    let member = guild_id.member(&ctx.http, actor_id).await?;
    Ok(member.roles.iter().any(|role_id| expected.contains(role_id)))
}

async fn validate_support_roles(
    ctx: &Context,
    guild_id: GuildId,
    support_role_ids: &[String],
) -> Result<()> {
    let guild = guild_id.to_partial_guild(&ctx.http).await?;
    for value in support_role_ids {
        let role_id = RoleId::new(parse_snowflake(value)?);
        if role_id.get() == guild_id.get() {
            return Err(anyhow!(
                "rola @everyone nie może być rolą wsparcia, ponieważ ujawniłaby prywatne tickety"
            ));
        }
        if !guild.roles.contains_key(&role_id) {
            return Err(anyhow!("rola wsparcia {role_id} nie istnieje na tym serwerze"));
        }
    }
    Ok(())
}

async fn ensure_category(
    ctx: &Context,
    guild_id: GuildId,
    category_id: &str,
) -> Result<ChannelId> {
    let category_id = ChannelId::new(parse_snowflake(category_id)?);
    match category_id.to_channel(ctx).await? {
        Channel::Guild(channel)
            if channel.guild_id == guild_id && channel.kind == ChannelType::Category =>
        {
            Ok(category_id)
        }
        Channel::Guild(_) => Err(anyhow!(
            "skonfigurowana kategoria musi należeć do tego serwera i mieć typ Category"
        )),
        _ => Err(anyhow!("kanał prywatny nie może być kategorią ticketów")),
    }
}

async fn resolve_optional_log_channel(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: Option<&str>,
) -> Result<Option<ChannelId>> {
    let Some(channel_id) = channel_id else {
        return Ok(None);
    };
    let channel_id = ChannelId::new(parse_snowflake(channel_id)?);
    match channel_id.to_channel(ctx).await? {
        Channel::Guild(channel)
            if channel.guild_id == guild_id
                && matches!(channel.kind, ChannelType::Text | ChannelType::News) =>
        {
            Ok(Some(channel_id))
        }
        Channel::Guild(_) => Err(anyhow!(
            "kanał logów ticketów musi być kanałem tekstowym tego serwera"
        )),
        _ => Err(anyhow!("kanał prywatny nie może być kanałem logów ticketów")),
    }
}

fn build_open_permissions(
    guild_id: GuildId,
    creator_id: UserId,
    bot_id: UserId,
    support_role_ids: &[String],
) -> Result<Vec<PermissionOverwrite>> {
    let mut permissions = vec![
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
            kind: PermissionOverwriteType::Role(RoleId::new(guild_id.get())),
        },
        PermissionOverwrite {
            allow: member_permissions(),
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(creator_id),
        },
        PermissionOverwrite {
            allow: bot_permissions(),
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(bot_id),
        },
    ];
    for role_id in support_role_ids {
        let role_id = RoleId::new(parse_snowflake(role_id)?);
        if role_id.get() == guild_id.get() {
            return Err(anyhow!("@everyone nie może otrzymać dostępu do ticketów"));
        }
        permissions.push(PermissionOverwrite {
            allow: staff_permissions(),
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Role(role_id),
        });
    }
    Ok(permissions)
}

async fn restore_member_access(ctx: &Context, channel_id: ChannelId, user_id: UserId) -> Result<()> {
    channel_id
        .create_permission(
            &ctx.http,
            PermissionOverwrite {
                allow: member_permissions(),
                deny: Permissions::empty(),
                kind: PermissionOverwriteType::Member(user_id),
            },
        )
        .await?;
    Ok(())
}

async fn set_member_read_only(ctx: &Context, channel_id: ChannelId, user_id: UserId) -> Result<()> {
    channel_id
        .create_permission(
            &ctx.http,
            PermissionOverwrite {
                allow: Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY,
                deny: Permissions::SEND_MESSAGES
                    | Permissions::SEND_MESSAGES_IN_THREADS
                    | Permissions::ADD_REACTIONS,
                kind: PermissionOverwriteType::Member(user_id),
            },
        )
        .await?;
    Ok(())
}

fn member_permissions() -> Permissions {
    Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::SEND_MESSAGES_IN_THREADS
        | Permissions::READ_MESSAGE_HISTORY
        | Permissions::ATTACH_FILES
        | Permissions::EMBED_LINKS
        | Permissions::ADD_REACTIONS
        | Permissions::USE_EXTERNAL_EMOJIS
}

fn staff_permissions() -> Permissions {
    member_permissions() | Permissions::MANAGE_MESSAGES
}

fn bot_permissions() -> Permissions {
    staff_permissions() | Permissions::MANAGE_CHANNELS
}

fn ensure_bot_permissions(actual: Permissions, required: Permissions) -> Result<()> {
    if actual.contains(Permissions::ADMINISTRATOR) || actual.contains(required) {
        Ok(())
    } else {
        Err(anyhow!(
            "bot nie ma wymaganych uprawnień do obsługi ticketu: {required:?}"
        ))
    }
}

async fn create_and_store_transcript(
    ctx: &Context,
    state: &AppState,
    ticket: &TicketRecord,
    actor_id: UserId,
    max_messages: u16,
) -> Result<crate::tickets::TicketTranscript> {
    let channel_id = ChannelId::new(parse_snowflake(
        ticket
            .channel_id
            .as_deref()
            .ok_or_else(|| anyhow!("ticket nie ma kanału"))?,
    )?);
    let (content, message_count) = build_transcript(ctx, channel_id, max_messages).await?;
    state
        .tickets
        .store_transcript(NewTicketTranscript {
            ticket_id: ticket.id.clone(),
            guild_id: ticket.guild_id.clone(),
            channel_id: channel_id.get().to_string(),
            generated_by_user_id: actor_id.get().to_string(),
            message_count,
            content,
        })
        .await
}

async fn build_transcript(
    ctx: &Context,
    channel_id: ChannelId,
    max_messages: u16,
) -> Result<(String, i32)> {
    let mut messages = Vec::with_capacity(usize::from(max_messages.min(1_000)));
    let mut before: Option<MessageId> = None;

    while messages.len() < usize::from(max_messages) {
        let remaining = usize::from(max_messages).saturating_sub(messages.len());
        let limit = u8::try_from(remaining.min(100)).unwrap_or(100);
        let mut request = GetMessages::new().limit(limit);
        if let Some(before) = before {
            request = request.before(before);
        }
        let batch = channel_id.messages(&ctx.http, request).await?;
        if batch.is_empty() {
            break;
        }
        before = batch.last().map(|message| message.id);
        let batch_length = batch.len();
        messages.extend(batch);
        if batch_length < usize::from(limit) {
            break;
        }
    }

    messages.sort_by_key(|message| message.id);
    let mut output = String::from("ZuckerBot ticket transcript\n============================\n");
    let mut included = 0_i32;
    for message in &messages {
        let line = format_transcript_message(message);
        if output.len().saturating_add(line.len()) > MAX_TRANSCRIPT_BYTES {
            output.push_str("\n[Transcript truncated because the safe attachment limit was reached.]\n");
            break;
        }
        output.push_str(&line);
        included = included.saturating_add(1);
    }
    Ok((output, included))
}

fn format_transcript_message(message: &Message) -> String {
    let content = if message.content.is_empty() {
        "[no text content]"
    } else {
        &message.content
    };
    let mut result = format!(
        "\n[{}] {} ({}) | message {}\n{}\n",
        message.timestamp,
        message.author.name,
        message.author.id.get(),
        message.id.get(),
        content,
    );
    for attachment in &message.attachments {
        result.push_str(&format!(
            "  attachment: {} | {}\n",
            attachment.filename, attachment.url
        ));
    }
    result
}

async fn send_transcript(
    ctx: &Context,
    channel_id: ChannelId,
    ticket: &TicketRecord,
    transcript: &crate::tickets::TicketTranscript,
) -> Result<()> {
    let filename = format!("ticket-{}-transcript.txt", ticket.display_number());
    channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content(format!(
                    "📄 Transkrypcja ticketu **#{}** — {} wiadomości.",
                    ticket.display_number(),
                    transcript.message_count
                ))
                .add_file(CreateAttachment::bytes(
                    transcript.content.clone().into_bytes(),
                    filename,
                ))
                .allowed_mentions(CreateAllowedMentions::new()),
        )
        .await?;
    Ok(())
}

async fn send_system_message(
    ctx: &Context,
    channel_id: ChannelId,
    content: &str,
    allowed_user: Option<UserId>,
) -> Result<()> {
    let mentions = match allowed_user {
        Some(user_id) => CreateAllowedMentions::new().users([user_id]),
        None => CreateAllowedMentions::new(),
    };
    channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content(content)
                .allowed_mentions(mentions),
        )
        .await?;
    Ok(())
}

async fn audit_ticket(
    state: &AppState,
    ticket: &TicketRecord,
    actor_id: Option<&str>,
    event: &str,
) -> Result<()> {
    state
        .storage
        .record_audit(
            Some(&ticket.guild_id),
            actor_id,
            "tickets",
            event,
            serde_json::json!({
                "ticket_id": ticket.id,
                "ticket_number": ticket.number,
                "channel_id": ticket.channel_id,
                "creator_user_id": ticket.creator_user_id,
                "claimed_by_user_id": ticket.claimed_by_user_id,
                "queue": ticket.queue,
                "priority": ticket.priority,
                "status": ticket.status,
                "version": ticket.version,
            }),
        )
        .await
}

fn format_welcome_message(ticket: &TicketRecord, custom: Option<&str>) -> String {
    format!(
        "<@{}> 🎫 **Ticket #{}**\n**Kolejka:** `{}`\n**Temat:** {}\n\n{}\n\n{}",
        ticket.creator_user_id,
        ticket.display_number(),
        ticket.queue,
        ticket.subject,
        ticket.description,
        custom.unwrap_or(
            "Zespół wsparcia odpowie w tym kanale. Nie publikuj haseł, tokenów ani innych sekretów."
        ),
    )
}

fn format_ticket_list(tickets: &[TicketRecord], include_owner: bool) -> String {
    if tickets.is_empty() {
        return "Brak aktywnych ticketów w wybranym zakresie.".to_owned();
    }
    let mut lines = vec![format!("🎫 **Aktywne tickety ({})**", tickets.len())];
    for ticket in tickets {
        let owner = if include_owner {
            format!(" · <@{}>", ticket.creator_user_id)
        } else {
            String::new()
        };
        let channel = ticket
            .channel_id
            .as_deref()
            .map(|id| format!("<#{}>", id))
            .unwrap_or_else(|| "provisioning".to_owned());
        lines.push(format!(
            "• **#{}** · {} · **{}** · `{}`{owner}\n  {} — {}",
            ticket.display_number(),
            priority_icon(&ticket.priority),
            status_label(&ticket.status),
            ticket.queue,
            channel,
            truncate(&ticket.subject, 72),
        ));
    }
    lines.join("\n")
}

fn format_ticket_info(
    ticket: &TicketRecord,
    participants: &[crate::tickets::TicketParticipant],
    events: &[crate::tickets::TicketEvent],
) -> String {
    let claimed = ticket
        .claimed_by_user_id
        .as_deref()
        .map(|id| format!("<@{id}>") )
        .unwrap_or_else(|| "nieprzejęty".to_owned());
    let participant_list = if participants.is_empty() {
        "brak".to_owned()
    } else {
        participants
            .iter()
            .map(|participant| format!("<@{}>", participant.user_id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let recent_events = if events.is_empty() {
        "brak".to_owned()
    } else {
        events
            .iter()
            .take(5)
            .map(|event| format!("`{}`", event.event_type))
            .collect::<Vec<_>>()
            .join(" → ")
    };
    format!(
        "🎫 **Ticket #{}** (`{}`)\n**Stan:** {}\n**Priorytet:** {} {}\n**Kolejka:** `{}`\n**Właściciel:** <@{}>\n**Obsługuje:** {}\n**Uczestnicy:** {}\n**Utworzono:** <t:{}:F>\n**Ostatnia aktywność:** <t:{}:R>\n**Temat:** {}\n**Ostatnie zdarzenia:** {}",
        ticket.display_number(),
        ticket.short_id(),
        status_label(&ticket.status),
        priority_icon(&ticket.priority),
        priority_label_from_str(&ticket.priority),
        ticket.queue,
        ticket.creator_user_id,
        claimed,
        participant_list,
        ticket.created_at.timestamp(),
        ticket.last_activity_at.timestamp(),
        ticket.subject,
        recent_events,
    )
}

fn format_mutation_outcome(outcome: TicketMutationOutcome) -> String {
    match outcome {
        TicketMutationOutcome::Updated(ticket) => format!(
            "Ticket #{} ma teraz stan **{}**.",
            ticket.display_number(),
            status_label(&ticket.status)
        ),
        TicketMutationOutcome::NotFound => "Nie znaleziono ticketu.".to_owned(),
        TicketMutationOutcome::Forbidden => {
            "Nie masz uprawnień do zmiany tego ticketu.".to_owned()
        }
        TicketMutationOutcome::InvalidState { current_status } => format!(
            "Ta operacja nie jest dozwolona dla ticketu w stanie **{}**.",
            status_label(&current_status)
        ),
        TicketMutationOutcome::Conflict { message } => {
            format!("Nie można wykonać operacji: {message}.")
        }
    }
}

fn format_participant_outcome(outcome: ParticipantMutationOutcome) -> String {
    match outcome {
        ParticipantMutationOutcome::Added(participant) => {
            format!("<@{}> jest już uczestnikiem.", participant.user_id)
        }
        ParticipantMutationOutcome::Removed => "Uczestnik został usunięty.".to_owned(),
        ParticipantMutationOutcome::NotFound => "Nie znaleziono uczestnika lub ticketu.".to_owned(),
        ParticipantMutationOutcome::AlreadyPresent => {
            "Ten użytkownik jest już uczestnikiem ticketu.".to_owned()
        }
        ParticipantMutationOutcome::CreatorProtected => {
            "Właściciel ticketu nie może zostać usunięty ani dodany jako zwykły uczestnik."
                .to_owned()
        }
        ParticipantMutationOutcome::InvalidState { current_status } => format!(
            "Nie można zmieniać uczestników ticketu w stanie **{}**.",
            status_label(&current_status)
        ),
    }
}

fn build_initial_channel_name(prefix: &str, number: i64, subject: &str) -> String {
    let slug = sanitize_channel_name(subject);
    truncate_channel_name(&format!("{}-{:06}-{}", prefix, number.max(0), slug))
}

fn build_closed_channel_name(ticket: &TicketRecord) -> String {
    let current = ticket
        .channel_name
        .as_deref()
        .unwrap_or("ticket")
        .trim_start_matches("closed-");
    truncate_channel_name(&format!("closed-{current}"))
}

fn remove_closed_prefix(value: &str) -> String {
    value.trim_start_matches("closed-").to_owned()
}

fn sanitize_channel_name(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(100));
    let mut previous_hyphen = false;
    for character in value.chars() {
        let normalized = if character.is_ascii_alphanumeric() {
            Some(character.to_ascii_lowercase())
        } else if character.is_whitespace() || matches!(character, '-' | '_' | '.') {
            Some('-')
        } else {
            None
        };
        if let Some(character) = normalized {
            if character == '-' {
                if previous_hyphen || output.is_empty() {
                    continue;
                }
                previous_hyphen = true;
            } else {
                previous_hyphen = false;
            }
            output.push(character);
        }
    }
    let output = output.trim_matches('-');
    if output.len() < 2 {
        "ticket".to_owned()
    } else {
        truncate_channel_name(output)
    }
}

fn truncate_channel_name(value: &str) -> String {
    let mut output = value.chars().take(100).collect::<String>();
    while output.ends_with('-') {
        output.pop();
    }
    if output.len() < 2 {
        "ticket".to_owned()
    } else {
        output
    }
}

fn truncate(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        value.to_owned()
    } else {
        value.chars().take(maximum.saturating_sub(3)).collect::<String>() + "..."
    }
}

fn status_label(status: &str) -> &str {
    match status {
        STATUS_PROVISIONING => "tworzenie",
        STATUS_OPEN => "otwarty",
        STATUS_CLAIMED => "przejęty",
        STATUS_CLOSED => "zamknięty",
        STATUS_FAILED => "błąd",
        _ => status,
    }
}

fn priority_label(priority: TicketPriority) -> &'static str {
    priority_label_from_str(priority.as_str())
}

fn priority_label_from_str(priority: &str) -> &str {
    match priority {
        "low" => "niski",
        "normal" => "normalny",
        "high" => "wysoki",
        "urgent" => "pilny",
        _ => priority,
    }
}

fn priority_icon(priority: &str) -> &str {
    match priority {
        "urgent" => "🔴",
        "high" => "🟠",
        "normal" => "🟡",
        "low" => "🟢",
        _ => "⚪",
    }
}

fn parse_snowflake(value: &str) -> Result<u64> {
    let value = value
        .parse::<u64>()
        .with_context(|| format!("{value:?} nie jest identyfikatorem Discorda"))?;
    if value == 0 {
        return Err(anyhow!("identyfikator Discorda nie może być zerem"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{build_initial_channel_name, sanitize_channel_name};

    #[test]
    fn channel_names_are_sanitized_and_bounded() {
        assert_eq!(sanitize_channel_name("Problem z PŁATNOŚCIĄ!!!"), "problem-z-patnoci");
        let name = build_initial_channel_name("ticket", 42, &"a".repeat(300));
        assert!(name.len() <= 100);
        assert!(name.starts_with("ticket-000042-"));
    }
}

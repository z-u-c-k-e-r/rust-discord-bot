use std::collections::HashMap;

use anyhow::{Context as _, Result, anyhow};
use chrono::{Duration, Utc};
use serenity::{
    all::{
        ChannelId, CommandInteraction, GuildId, Member, MessageId, Permissions, Role, RoleId,
        UserId,
    },
    builder::{
        CreateAllowedMentions, CreateInteractionResponse, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage, CreateMessage, EditMember, GetMessages,
    },
    client::Context,
};

use crate::{AppState, lua::LuaAction};

use super::{music, progression, scheduler};

struct ActionOrigin {
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    app_permissions: Permissions,
    enforce_actor_permissions: bool,
}

pub async fn execute_command_actions(
    ctx: &Context,
    state: &AppState,
    command: &CommandInteraction,
    module_id: &str,
    actions: &[LuaAction],
) -> Result<()> {
    let first_reply = actions.iter().position(|action| {
        matches!(
            action,
            LuaAction::Reply {
                content: _,
                ephemeral: _
            }
        )
    });

    if let Some(index) = first_reply {
        if let LuaAction::Reply { content, ephemeral } = &actions[index] {
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(content)
                            .ephemeral(*ephemeral)
                            .allowed_mentions(CreateAllowedMentions::new()),
                    ),
                )
                .await?;
        }
    } else {
        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Polecenie zostało przyjęte.")
                        .ephemeral(true)
                        .allowed_mentions(CreateAllowedMentions::new()),
                ),
            )
            .await?;
    }

    let origin = ActionOrigin {
        guild_id: command.guild_id,
        channel_id: Some(command.channel_id),
        actor_id: Some(command.user.id),
        actor_permissions: command
            .member
            .as_ref()
            .and_then(|member| member.permissions)
            .unwrap_or_else(Permissions::empty),
        app_permissions: command.app_permissions.unwrap_or_else(Permissions::empty),
        enforce_actor_permissions: true,
    };

    for (index, action) in actions.iter().enumerate() {
        if Some(index) == first_reply {
            continue;
        }

        if let LuaAction::Reply { content, ephemeral } = action {
            command
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .content(content)
                        .ephemeral(*ephemeral)
                        .allowed_mentions(CreateAllowedMentions::new()),
                )
                .await?;
            continue;
        }

        match execute_action(ctx, state, module_id, &origin, action).await {
            Ok(Some(status)) => {
                command
                    .create_followup(
                        &ctx.http,
                        CreateInteractionResponseFollowup::new()
                            .content(status)
                            .ephemeral(true)
                            .allowed_mentions(CreateAllowedMentions::new()),
                    )
                    .await?;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    module_id,
                    command = %command.data.name,
                    user_id = %command.user.id,
                    ?error,
                    "Lua action failed"
                );
                command
                    .create_followup(
                        &ctx.http,
                        CreateInteractionResponseFollowup::new()
                            .content(format!("Nie udało się wykonać akcji: {error}"))
                            .ephemeral(true)
                            .allowed_mentions(CreateAllowedMentions::new()),
                    )
                    .await?;
                break;
            }
        }
    }

    Ok(())
}

pub async fn execute_event_actions(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actions: &[LuaAction],
) -> Result<()> {
    let origin = ActionOrigin {
        guild_id,
        channel_id,
        actor_id,
        actor_permissions: Permissions::empty(),
        app_permissions: Permissions::all(),
        enforce_actor_permissions: false,
    };

    for action in actions {
        match action {
            LuaAction::Reply { content, .. } => {
                let channel_id =
                    channel_id.ok_or_else(|| anyhow!("event reply has no channel context"))?;
                send_message(ctx, channel_id, content).await?;
            }
            _ => {
                execute_action(ctx, state, module_id, &origin, action).await?;
            }
        }
    }

    Ok(())
}

async fn execute_action(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    origin: &ActionOrigin,
    action: &LuaAction,
) -> Result<Option<String>> {
    if let Some(required) = required_permission(action) {
        if origin.enforce_actor_permissions && !has_permission(origin.actor_permissions, required) {
            return Err(anyhow!(
                "wywołujący nie ma wymaganych uprawnień: {required:?}"
            ));
        }
        if origin.enforce_actor_permissions && !has_permission(origin.app_permissions, required) {
            return Err(anyhow!("bot nie ma wymaganych uprawnień: {required:?}"));
        }
    }

    match action {
        LuaAction::Reply { .. } => Ok(None),
        LuaAction::SendMessage {
            channel_id,
            content,
        } => {
            let channel_id = resolve_channel(channel_id.as_deref(), origin.channel_id)?;
            send_message(ctx, channel_id, content).await?;
            Ok(None)
        }
        LuaAction::DeleteMessage {
            channel_id,
            message_id,
        } => {
            ChannelId::new(parse_snowflake(channel_id)?)
                .delete_message(&ctx.http, MessageId::new(parse_snowflake(message_id)?))
                .await?;
            Ok(None)
        }
        LuaAction::TimeoutMember {
            user_id,
            seconds,
            reason,
        } => {
            let guild_id = require_guild(origin)?;
            let target_id = UserId::new(parse_snowflake(user_id)?);
            ensure_member_hierarchy(ctx, guild_id, origin.actor_id, target_id).await?;
            let until = (Utc::now() + Duration::seconds(i64::try_from(*seconds)?)).to_rfc3339();
            guild_id
                .edit_member(
                    &ctx.http,
                    target_id,
                    EditMember::new().disable_communication_until(until),
                )
                .await?;
            audit_action(
                state,
                origin,
                module_id,
                "timeout_member",
                serde_json::json!({
                    "target_user_id": user_id,
                    "seconds": seconds,
                    "reason": reason,
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::KickMember { user_id, reason } => {
            let guild_id = require_guild(origin)?;
            let target_id = UserId::new(parse_snowflake(user_id)?);
            ensure_member_hierarchy(ctx, guild_id, origin.actor_id, target_id).await?;
            guild_id
                .kick_with_reason(
                    &ctx.http,
                    target_id,
                    reason.as_deref().unwrap_or("ZuckerBot Lua module"),
                )
                .await?;
            audit_action(
                state,
                origin,
                module_id,
                "kick_member",
                serde_json::json!({
                    "target_user_id": user_id,
                    "reason": reason,
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::BanMember {
            user_id,
            delete_message_days,
            reason,
        } => {
            let guild_id = require_guild(origin)?;
            let target_id = UserId::new(parse_snowflake(user_id)?);
            ensure_member_hierarchy(ctx, guild_id, origin.actor_id, target_id).await?;
            guild_id
                .ban_with_reason(
                    &ctx.http,
                    target_id,
                    *delete_message_days,
                    reason.as_deref().unwrap_or("ZuckerBot Lua module"),
                )
                .await?;
            audit_action(
                state,
                origin,
                module_id,
                "ban_member",
                serde_json::json!({
                    "target_user_id": user_id,
                    "delete_message_days": delete_message_days,
                    "reason": reason,
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::AddRole {
            user_id,
            role_id,
            reason,
        } => {
            let guild_id = require_guild(origin)?;
            let user_id = UserId::new(parse_snowflake(user_id)?);
            let role_id = RoleId::new(parse_snowflake(role_id)?);
            ensure_role_hierarchy(ctx, guild_id, origin.actor_id, role_id).await?;
            let member = guild_id.member(&ctx.http, user_id).await?;
            member.add_role(&ctx.http, role_id).await?;
            audit_action(
                state,
                origin,
                module_id,
                "add_role",
                serde_json::json!({
                    "target_user_id": user_id,
                    "role_id": role_id,
                    "reason": reason,
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::RemoveRole {
            user_id,
            role_id,
            reason,
        } => {
            let guild_id = require_guild(origin)?;
            let user_id = UserId::new(parse_snowflake(user_id)?);
            let role_id = RoleId::new(parse_snowflake(role_id)?);
            ensure_role_hierarchy(ctx, guild_id, origin.actor_id, role_id).await?;
            let member = guild_id.member(&ctx.http, user_id).await?;
            member.remove_role(&ctx.http, role_id).await?;
            audit_action(
                state,
                origin,
                module_id,
                "remove_role",
                serde_json::json!({
                    "target_user_id": user_id,
                    "role_id": role_id,
                    "reason": reason,
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::Purge { channel_id, amount } => {
            let channel_id = resolve_channel(channel_id.as_deref(), origin.channel_id)?;
            let messages = channel_id
                .messages(&ctx.http, GetMessages::new().limit(*amount))
                .await?;
            let message_ids = messages
                .iter()
                .map(|message| message.id)
                .collect::<Vec<_>>();
            if message_ids.len() == 1 {
                channel_id.delete_message(&ctx.http, message_ids[0]).await?;
            } else if !message_ids.is_empty() {
                channel_id.delete_messages(&ctx.http, message_ids).await?;
            }
            audit_action(
                state,
                origin,
                module_id,
                "purge",
                serde_json::json!({
                    "channel_id": channel_id,
                    "requested_amount": amount,
                    "deleted_amount": messages.len(),
                }),
            )
            .await?;
            Ok(None)
        }
        LuaAction::Music { operation, query } => {
            let guild_id = require_guild(origin)?;
            let actor_id = origin
                .actor_id
                .ok_or_else(|| anyhow!("music actions require a user context"))?;
            let status =
                music::execute(ctx, state, guild_id, actor_id, *operation, query.as_deref())
                    .await?;
            audit_action(
                state,
                origin,
                module_id,
                "music",
                serde_json::json!({
                    "operation": operation,
                    "query": query,
                }),
            )
            .await?;
            Ok(Some(status))
        }
        LuaAction::Progression { operation } => {
            progression::execute(
                ctx,
                state,
                module_id,
                progression::ProgressionExecutionContext::new(
                    origin.guild_id,
                    origin.channel_id,
                    origin.actor_id,
                    origin.actor_permissions,
                    origin.enforce_actor_permissions,
                ),
                operation,
            )
            .await
        }
        LuaAction::Scheduler { operation } => {
            scheduler::execute(
                ctx,
                state,
                module_id,
                scheduler::SchedulerExecutionContext::new(
                    origin.guild_id,
                    origin.channel_id,
                    origin.actor_id,
                    origin.actor_permissions,
                    origin.enforce_actor_permissions,
                ),
                operation,
            )
            .await
        }
        LuaAction::Audit { event, data } => {
            state
                .storage
                .record_audit(
                    origin.guild_id.map(|id| id.get().to_string()).as_deref(),
                    origin.actor_id.map(|id| id.get().to_string()).as_deref(),
                    module_id,
                    event,
                    serde_json::to_value(data)?,
                )
                .await?;
            Ok(None)
        }
    }
}

async fn send_message(ctx: &Context, channel_id: ChannelId, content: &str) -> Result<()> {
    channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new()
                .content(content)
                .allowed_mentions(CreateAllowedMentions::new()),
        )
        .await?;
    Ok(())
}

fn required_permission(action: &LuaAction) -> Option<Permissions> {
    match action {
        LuaAction::DeleteMessage { .. } | LuaAction::Purge { .. } => {
            Some(Permissions::MANAGE_MESSAGES)
        }
        LuaAction::TimeoutMember { .. } => Some(Permissions::MODERATE_MEMBERS),
        LuaAction::KickMember { .. } => Some(Permissions::KICK_MEMBERS),
        LuaAction::BanMember { .. } => Some(Permissions::BAN_MEMBERS),
        LuaAction::AddRole { .. } | LuaAction::RemoveRole { .. } => Some(Permissions::MANAGE_ROLES),
        _ => None,
    }
}

fn has_permission(actual: Permissions, required: Permissions) -> bool {
    actual.contains(Permissions::ADMINISTRATOR) || actual.contains(required)
}

fn require_guild(origin: &ActionOrigin) -> Result<GuildId> {
    origin
        .guild_id
        .ok_or_else(|| anyhow!("ta akcja może zostać wykonana wyłącznie na serwerze"))
}

fn resolve_channel(value: Option<&str>, fallback: Option<ChannelId>) -> Result<ChannelId> {
    match value {
        Some(value) => Ok(ChannelId::new(parse_snowflake(value)?)),
        None => fallback.ok_or_else(|| anyhow!("brakuje kanału dla tej akcji")),
    }
}

fn parse_snowflake(value: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{value:?} nie jest prawidłowym identyfikatorem Discorda"))?;
    if parsed == 0 {
        return Err(anyhow!("identyfikator Discorda nie może być zerem"));
    }
    Ok(parsed)
}

async fn ensure_member_hierarchy(
    ctx: &Context,
    guild_id: GuildId,
    actor_id: Option<UserId>,
    target_id: UserId,
) -> Result<()> {
    let guild = guild_id.to_partial_guild(&ctx.http).await?;
    if target_id == guild.owner_id {
        return Err(anyhow!("właściciel serwera nie może zostać moderowany"));
    }

    let target = guild_id.member(&ctx.http, target_id).await?;
    let bot_id = ctx.cache.current_user().id;
    let bot = guild_id.member(&ctx.http, bot_id).await?;
    let target_rank = member_rank(&target, &guild.roles);
    let bot_rank = member_rank(&bot, &guild.roles);

    if !rank_is_higher(bot_rank, target_rank) {
        return Err(anyhow!(
            "najwyższa rola bota musi znajdować się nad rolą użytkownika"
        ));
    }

    if let Some(actor_id) = actor_id {
        if actor_id == target_id {
            return Err(anyhow!("nie możesz wykonać tej akcji na sobie"));
        }
        if actor_id != guild.owner_id {
            let actor = guild_id.member(&ctx.http, actor_id).await?;
            if !rank_is_higher(member_rank(&actor, &guild.roles), target_rank) {
                return Err(anyhow!(
                    "twoja najwyższa rola musi znajdować się nad rolą użytkownika"
                ));
            }
        }
    }

    Ok(())
}

async fn ensure_role_hierarchy(
    ctx: &Context,
    guild_id: GuildId,
    actor_id: Option<UserId>,
    role_id: RoleId,
) -> Result<()> {
    let guild = guild_id.to_partial_guild(&ctx.http).await?;
    let role = guild
        .roles
        .get(&role_id)
        .ok_or_else(|| anyhow!("rola nie istnieje na tym serwerze"))?;
    let target_rank = role_rank(role);

    let bot_id = ctx.cache.current_user().id;
    let bot = guild_id.member(&ctx.http, bot_id).await?;
    if !rank_is_higher(member_rank(&bot, &guild.roles), target_rank) {
        return Err(anyhow!(
            "najwyższa rola bota musi znajdować się nad zarządzaną rolą"
        ));
    }

    if let Some(actor_id) = actor_id
        && actor_id != guild.owner_id
    {
        let actor = guild_id.member(&ctx.http, actor_id).await?;
        if !rank_is_higher(member_rank(&actor, &guild.roles), target_rank) {
            return Err(anyhow!(
                "twoja najwyższa rola musi znajdować się nad zarządzaną rolą"
            ));
        }
    }

    Ok(())
}

fn member_rank(member: &Member, roles: &HashMap<RoleId, Role>) -> (u16, u64) {
    member
        .roles
        .iter()
        .filter_map(|role_id| roles.get(role_id))
        .map(role_rank)
        .max_by(|left, right| compare_rank(*left, *right))
        .unwrap_or((0, member.guild_id.get()))
}

fn role_rank(role: &Role) -> (u16, u64) {
    (role.position, role.id.get())
}

fn rank_is_higher(left: (u16, u64), right: (u16, u64)) -> bool {
    compare_rank(left, right).is_gt()
}

fn compare_rank(left: (u16, u64), right: (u16, u64)) -> std::cmp::Ordering {
    left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1))
}

async fn audit_action(
    state: &AppState,
    origin: &ActionOrigin,
    module_id: &str,
    event: &str,
    data: serde_json::Value,
) -> Result<()> {
    let guild_id = origin.guild_id.map(|id| id.get().to_string());
    let actor_id = origin.actor_id.map(|id| id.get().to_string());
    state
        .storage
        .record_audit(
            guild_id.as_deref(),
            actor_id.as_deref(),
            module_id,
            event,
            data,
        )
        .await
}

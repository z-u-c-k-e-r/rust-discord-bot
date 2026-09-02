use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Map, Value, json};
use serenity::{
    all::{
        CommandDataOption, CommandDataOptionValue, CommandInteraction, Interaction, Member,
        Message, Ready,
    },
    async_trait,
    builder::{CreateAllowedMentions, CreateInteractionResponse, CreateInteractionResponseMessage},
    client::{Context, EventHandler},
};

use crate::{
    AppState,
    lua::{LuaEventContext, LuaExecutionContext},
};

use super::{commands, executor};

pub struct Handler {
    state: AppState,
    commands_registered: AtomicBool,
}

impl Handler {
    pub const fn new(state: AppState) -> Self {
        Self {
            state,
            commands_registered: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!(
            user = %ready.user.name,
            guilds = ready.guilds.len(),
            "Discord gateway is ready"
        );

        if !self.commands_registered.swap(true, Ordering::AcqRel)
            && let Err(error) = commands::register(&ctx, &self.state).await
        {
            self.commands_registered.store(false, Ordering::Release);
            tracing::error!(?error, "failed to register application commands");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        if let Err(error) = handle_command(&ctx, &self.state, &command).await {
            tracing::error!(
                command = %command.data.name,
                user_id = %command.user.id,
                ?error,
                "failed to handle application command"
            );
            let _ = respond_ephemeral(
                &ctx,
                &command,
                "Wewnętrzny błąd bota. Szczegóły zostały zapisane w logach.",
            )
            .await;
        }
    }

    async fn message(&self, ctx: Context, message: Message) {
        if message.author.bot {
            return;
        }
        let Some(guild_id) = message.guild_id else {
            return;
        };

        let event = LuaEventContext {
            name: "message_create".to_owned(),
            guild_id: Some(guild_id.get().to_string()),
            channel_id: Some(message.channel_id.get().to_string()),
            actor_id: Some(message.author.id.get().to_string()),
            data: json!({
                "message_id": message.id,
                "content": message.content,
                "author": {
                    "id": message.author.id,
                    "name": message.author.name,
                    "global_name": message.author.global_name,
                },
                "mentions": message.mentions.iter().map(|user| user.id).collect::<Vec<_>>(),
                "attachments": message.attachments.iter().map(|attachment| {
                    json!({
                        "id": attachment.id,
                        "filename": attachment.filename,
                        "size": attachment.size,
                        "content_type": attachment.content_type,
                        "url": attachment.url,
                    })
                }).collect::<Vec<_>>(),
            }),
            config: Value::Null,
        };

        run_event(
            &ctx,
            &self.state,
            "message_create",
            event,
            Some(guild_id),
            Some(message.channel_id),
            Some(message.author.id),
        )
        .await;
    }

    async fn guild_member_addition(&self, ctx: Context, new_member: Member) {
        let guild_id = new_member.guild_id;
        let event = LuaEventContext {
            name: "guild_member_add".to_owned(),
            guild_id: Some(guild_id.get().to_string()),
            channel_id: None,
            actor_id: Some(new_member.user.id.get().to_string()),
            data: json!({
                "user": {
                    "id": new_member.user.id,
                    "name": new_member.user.name,
                    "global_name": new_member.user.global_name,
                    "bot": new_member.user.bot,
                },
                "roles": new_member.roles,
                "joined_at": new_member.joined_at,
            }),
            config: Value::Null,
        };

        run_event(
            &ctx,
            &self.state,
            "guild_member_add",
            event,
            Some(guild_id),
            None,
            Some(new_member.user.id),
        )
        .await;
    }
}

async fn handle_command(
    ctx: &Context,
    state: &AppState,
    command: &CommandInteraction,
) -> anyhow::Result<()> {
    let Some(module_id) = state.scripts.module_for_command(&command.data.name).await else {
        respond_ephemeral(ctx, command, "Ta komenda nie jest już dostępna.").await?;
        return Ok(());
    };
    let Some(manifest) = state.scripts.manifest(&module_id).await else {
        respond_ephemeral(ctx, command, "Moduł tej komendy nie jest załadowany.").await?;
        return Ok(());
    };

    let (enabled, config) = match command.guild_id {
        Some(guild_id) => match state
            .storage
            .get_module_settings(&guild_id.get().to_string(), &module_id)
            .await?
        {
            Some(settings) => (settings.enabled, settings.config),
            None => (manifest.default_enabled, json!({})),
        },
        None => (manifest.default_enabled, json!({})),
    };

    if !enabled {
        respond_ephemeral(
            ctx,
            command,
            "Ten moduł został wyłączony w panelu administracyjnym.",
        )
        .await?;
        return Ok(());
    }

    let member_roles = command
        .member
        .as_ref()
        .map(|member| {
            member
                .roles
                .iter()
                .map(|role_id| role_id.get().to_string())
                .collect()
        })
        .unwrap_or_default();
    let member_permissions = command
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .map_or_else(
            || "0".to_owned(),
            |permissions| permissions.bits().to_string(),
        );

    let execution_context = LuaExecutionContext {
        guild_id: command.guild_id.map(|id| id.get().to_string()),
        channel_id: command.channel_id.get().to_string(),
        user_id: command.user.id.get().to_string(),
        user_name: command
            .user
            .global_name
            .clone()
            .unwrap_or_else(|| command.user.name.clone()),
        member_roles,
        member_permissions,
        locale: command.locale.clone(),
        options: options_to_json(&command.data.options),
        config,
    };

    let actions = state
        .scripts
        .execute_command(&module_id, &command.data.name, execution_context)
        .await?;
    executor::execute_command_actions(ctx, state, command, &module_id, &actions).await
}

async fn run_event(
    ctx: &Context,
    state: &AppState,
    event_name: &str,
    event: LuaEventContext,
    guild_id: Option<serenity::all::GuildId>,
    channel_id: Option<serenity::all::ChannelId>,
    actor_id: Option<serenity::all::UserId>,
) {
    for module_id in state.scripts.modules_for_event(event_name).await {
        let Some(manifest) = state.scripts.manifest(&module_id).await else {
            continue;
        };

        let mut module_event = event.clone();
        if let Some(guild_id) = guild_id {
            match state
                .storage
                .get_module_settings(&guild_id.get().to_string(), &module_id)
                .await
            {
                Ok(Some(settings)) if settings.enabled => {
                    module_event.config = settings.config;
                }
                Ok(Some(_)) => continue,
                Ok(None) if manifest.default_enabled => {
                    module_event.config = json!({});
                }
                Ok(None) => continue,
                Err(error) => {
                    tracing::error!(module_id, event_name, ?error, "cannot load module settings");
                    continue;
                }
            }
        }

        match state
            .scripts
            .execute_event(&module_id, event_name, module_event)
            .await
        {
            Ok(actions) => {
                if let Err(error) = executor::execute_event_actions(
                    ctx, state, &module_id, guild_id, channel_id, actor_id, &actions,
                )
                .await
                {
                    tracing::warn!(module_id, event_name, ?error, "event action failed");
                }
            }
            Err(error) => {
                tracing::warn!(module_id, event_name, ?error, "Lua event failed");
            }
        }
    }
}

fn options_to_json(options: &[CommandDataOption]) -> Value {
    let mut object = Map::new();
    for option in options {
        object.insert(option.name.clone(), option_value_to_json(&option.value));
    }
    Value::Object(object)
}

fn option_value_to_json(value: &CommandDataOptionValue) -> Value {
    match value {
        CommandDataOptionValue::Autocomplete { value, .. }
        | CommandDataOptionValue::String(value) => Value::String(value.clone()),
        CommandDataOptionValue::Boolean(value) => Value::Bool(*value),
        CommandDataOptionValue::Integer(value) => json!(value),
        CommandDataOptionValue::Number(value) => json!(value),
        CommandDataOptionValue::SubCommand(options)
        | CommandDataOptionValue::SubCommandGroup(options) => options_to_json(options),
        CommandDataOptionValue::Attachment(value) => Value::String(value.get().to_string()),
        CommandDataOptionValue::Channel(value) => Value::String(value.get().to_string()),
        CommandDataOptionValue::Mentionable(value) => Value::String(value.get().to_string()),
        CommandDataOptionValue::Role(value) => Value::String(value.get().to_string()),
        CommandDataOptionValue::User(value) => Value::String(value.get().to_string()),
        CommandDataOptionValue::Unknown(value) => json!(value),
        _ => Value::Null,
    }
}

async fn respond_ephemeral(
    ctx: &Context,
    command: &CommandInteraction,
    content: &str,
) -> serenity::Result<()> {
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true)
                    .allowed_mentions(CreateAllowedMentions::new()),
            ),
        )
        .await
}

use std::{collections::HashMap, sync::Arc};

use anyhow::{Context as _, Result, bail};
use reqwest::Client as HttpClient;
use serde_json::{Value, json};
use serenity::{
    all::{
        ChannelId, Command, CommandDataOption, CommandDataOptionValue, CommandInteraction,
        CommandOptionType, Context, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, CreateInteractionResponseMessage, EventHandler, GuildId,
        Interaction, Permissions, Ready, UserId,
    },
    async_trait,
};
use songbird::input::YoutubeDl;

use crate::{
    config::Config,
    lua::LuaEngine,
    model::{
        CommandContext, CommandManifest, CommandOptionKind, LuaAction, ModuleManifest,
    },
    storage::Storage,
};

#[derive(Clone)]
pub struct DiscordHandler {
    config: Arc<Config>,
    engine: LuaEngine,
    storage: Storage,
    http_client: HttpClient,
}

impl DiscordHandler {
    pub fn new(
        config: Arc<Config>,
        engine: LuaEngine,
        storage: Storage,
        http_client: HttpClient,
    ) -> Self {
        Self {
            config,
            engine,
            storage,
            http_client,
        }
    }

    async fn register_commands(&self, context: &Context) -> Result<()> {
        let commands = self
            .engine
            .manifests()
            .iter()
            .flat_map(commands_from_module)
            .collect::<Result<Vec<_>>>()?;

        let installed = Command::set_global_commands(&context.http, commands).await?;
        tracing::info!(count = installed.len(), "global slash commands synchronized");
        Ok(())
    }

    async fn handle_command(
        &self,
        context: &Context,
        interaction: &CommandInteraction,
    ) -> Result<()> {
        let resolved = self
            .engine
            .find_command(&interaction.data.name)
            .with_context(|| format!("unknown Lua command {}", interaction.data.name))?;

        if let Some(guild_id) = interaction.guild_id {
            if !self
                .storage
                .module_enabled(guild_id.get(), &resolved.module_name)
                .await?
            {
                return reply(
                    context,
                    interaction,
                    "Ten moduł jest wyłączony na tym serwerze.",
                    true,
                )
                .await;
            }
        }

        enforce_manifest_permissions(interaction, &resolved.command)?;

        let module_config = match interaction.guild_id {
            Some(guild_id) => {
                self.storage
                    .module_config(guild_id.get(), &resolved.module_name)
                    .await?
            }
            None => Value::Null,
        };

        let lua_context = CommandContext {
            command: interaction.data.name.clone(),
            guild_id: interaction.guild_id.map(|id| id.get().to_string()),
            channel_id: interaction.channel_id.get().to_string(),
            user_id: interaction.user.id.get().to_string(),
            username: interaction
                .user
                .global_name
                .clone()
                .unwrap_or_else(|| interaction.user.name.clone()),
            options: collect_options(&interaction.data.options),
            module_config,
        };

        let engine = self.engine.clone();
        let module_name = resolved.module_name;
        let actions = tokio::task::spawn_blocking(move || engine.execute(&module_name, lua_context))
            .await
            .context("Lua worker panicked")??;

        self.execute_actions(context, interaction, actions).await
    }

    async fn execute_actions(
        &self,
        context: &Context,
        interaction: &CommandInteraction,
        actions: Vec<LuaAction>,
    ) -> Result<()> {
        let mut responded = false;

        for action in actions {
            match action {
                LuaAction::Reply { content, ephemeral } => {
                    if responded {
                        interaction
                            .channel_id
                            .say(&context.http, truncate(&content, 2_000))
                            .await?;
                    } else {
                        reply(context, interaction, &content, ephemeral).await?;
                        responded = true;
                    }
                }
                LuaAction::SendMessage { content } => {
                    interaction
                        .channel_id
                        .say(&context.http, truncate(&content, 2_000))
                        .await?;
                }
                LuaAction::Kick { user_id, reason } => {
                    require_action_permission(interaction, Permissions::KICK_MEMBERS)?;
                    let guild_id = required_guild(interaction)?;
                    guild_id
                        .kick_with_reason(&context.http, parse_user_id(&user_id)?, &reason)
                        .await?;
                }
                LuaAction::Ban {
                    user_id,
                    reason,
                    delete_message_seconds,
                } => {
                    require_action_permission(interaction, Permissions::BAN_MEMBERS)?;
                    let guild_id = required_guild(interaction)?;
                    guild_id
                        .ban_with_reason(
                            &context.http,
                            parse_user_id(&user_id)?,
                            delete_message_seconds,
                            &reason,
                        )
                        .await?;
                }
                LuaAction::VoiceJoin => {
                    let guild_id = required_guild(interaction)?;
                    self.join_requester_channel(context, interaction, guild_id)
                        .await?;
                }
                LuaAction::VoiceLeave => {
                    let guild_id = required_guild(interaction)?;
                    let manager = songbird_manager(context).await?;
                    manager.remove(guild_id).await?;
                }
                LuaAction::MusicPlay { query } => {
                    let guild_id = required_guild(interaction)?;
                    let manager = songbird_manager(context).await?;
                    let call = match manager.get(guild_id) {
                        Some(call) => call,
                        None => {
                            self.join_requester_channel(context, interaction, guild_id)
                                .await?
                        }
                    };

                    let source = if query.starts_with("http://") || query.starts_with("https://") {
                        YoutubeDl::new(self.http_client.clone(), query)
                    } else {
                        YoutubeDl::new_search(self.http_client.clone(), query)
                    };
                    call.lock().await.enqueue_input(source.into()).await;
                }
                LuaAction::MusicPause => {
                    let call = active_call(context, required_guild(interaction)?).await?;
                    call.lock().await.queue().pause()?;
                }
                LuaAction::MusicResume => {
                    let call = active_call(context, required_guild(interaction)?).await?;
                    call.lock().await.queue().resume()?;
                }
                LuaAction::MusicSkip => {
                    let call = active_call(context, required_guild(interaction)?).await?;
                    call.lock().await.queue().skip()?;
                }
                LuaAction::MusicStop => {
                    let call = active_call(context, required_guild(interaction)?).await?;
                    call.lock().await.queue().stop();
                }
            }
        }

        if !responded {
            reply(context, interaction, "Wykonano.", true).await?;
        }
        Ok(())
    }

    async fn join_requester_channel(
        &self,
        context: &Context,
        interaction: &CommandInteraction,
        guild_id: GuildId,
    ) -> Result<Arc<tokio::sync::Mutex<songbird::Call>>> {
        let channel_id = {
            let guild = context
                .cache
                .guild(guild_id)
                .context("server is not available in the Discord cache")?;
            guild
                .voice_states
                .get(&interaction.user.id)
                .and_then(|state| state.channel_id)
                .context("musisz najpierw wejść na kanał głosowy")?
        };

        let manager = songbird_manager(context).await?;
        let call = manager.join(guild_id, channel_id).await?;
        Ok(call)
    }
}

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn ready(&self, context: Context, ready: Ready) {
        tracing::info!(user = %ready.user.name, "Discord gateway connected");
        if let Err(error) = self.register_commands(&context).await {
            tracing::error!(%error, "failed to synchronize slash commands");
        }
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        if let Err(error) = self.handle_command(&context, &command).await {
            tracing::error!(command = %command.data.name, %error, "command failed");
            let message = format!("Błąd: {}", truncate(&error.to_string(), 1_800));
            if let Err(response_error) = reply(&context, &command, &message, true).await {
                tracing::warn!(%response_error, "failed to return command error to Discord");
            }
        }
    }
}

fn commands_from_module(module: &ModuleManifest) -> impl Iterator<Item = Result<CreateCommand>> + '_ {
    module.commands.iter().map(|command| {
        let permissions = permissions_from_names(&command.required_permissions)?;
        let mut builder = CreateCommand::new(&command.name)
            .description(&command.description)
            .dm_permission(command.dm_permission)
            .nsfw(command.nsfw);

        if !permissions.is_empty() {
            builder = builder.default_member_permissions(permissions);
        }

        for option in &command.options {
            let kind = match option.kind {
                CommandOptionKind::String => CommandOptionType::String,
                CommandOptionKind::Integer => CommandOptionType::Integer,
                CommandOptionKind::Number => CommandOptionType::Number,
                CommandOptionKind::Boolean => CommandOptionType::Boolean,
                CommandOptionKind::User => CommandOptionType::User,
                CommandOptionKind::Channel => CommandOptionType::Channel,
                CommandOptionKind::Role => CommandOptionType::Role,
                CommandOptionKind::Mentionable => CommandOptionType::Mentionable,
                CommandOptionKind::Attachment => CommandOptionType::Attachment,
            };

            let mut option_builder = CreateCommandOption::new(kind, &option.name, &option.description)
                .required(option.required);
            if let Some(value) = option.min_integer {
                option_builder = option_builder.min_int_value(value);
            }
            if let Some(value) = option.max_integer {
                option_builder = option_builder.max_int_value(value);
            }
            if let Some(value) = option.min_length {
                option_builder = option_builder.min_length(value);
            }
            if let Some(value) = option.max_length {
                option_builder = option_builder.max_length(value);
            }
            builder = builder.add_option(option_builder);
        }

        Ok(builder)
    })
}

fn collect_options(options: &[CommandDataOption]) -> HashMap<String, Value> {
    options
        .iter()
        .map(|option| (option.name.clone(), option_value(&option.value)))
        .collect()
}

fn option_value(value: &CommandDataOptionValue) -> Value {
    match value {
        CommandDataOptionValue::Boolean(value) => json!(value),
        CommandDataOptionValue::Integer(value) => json!(value),
        CommandDataOptionValue::Number(value) => json!(value),
        CommandDataOptionValue::String(value) => json!(value),
        CommandDataOptionValue::User(value) => json!(value.get().to_string()),
        CommandDataOptionValue::Channel(value) => json!(value.get().to_string()),
        CommandDataOptionValue::Role(value) => json!(value.get().to_string()),
        CommandDataOptionValue::Mentionable(value) => json!(value.get().to_string()),
        CommandDataOptionValue::Attachment(value) => json!(value.get().to_string()),
        CommandDataOptionValue::SubCommand(options)
        | CommandDataOptionValue::SubCommandGroup(options) => {
            json!(collect_options(options))
        }
        _ => Value::Null,
    }
}

fn enforce_manifest_permissions(
    interaction: &CommandInteraction,
    command: &CommandManifest,
) -> Result<()> {
    let required = permissions_from_names(&command.required_permissions)?;
    if required.is_empty() {
        return Ok(());
    }

    let granted = interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .unwrap_or_else(Permissions::empty);
    if !granted.contains(required) {
        bail!("nie masz wymaganych uprawnień: {required:?}");
    }
    Ok(())
}

fn require_action_permission(interaction: &CommandInteraction, required: Permissions) -> Result<()> {
    let granted = interaction
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .unwrap_or_else(Permissions::empty);
    if !granted.contains(required) {
        bail!("akcja Lua wymaga uprawnienia {required:?}");
    }
    Ok(())
}

fn permissions_from_names(names: &[String]) -> Result<Permissions> {
    names.iter().try_fold(Permissions::empty(), |permissions, name| {
        let permission = match name.as_str() {
            "administrator" => Permissions::ADMINISTRATOR,
            "manage_guild" => Permissions::MANAGE_GUILD,
            "manage_messages" => Permissions::MANAGE_MESSAGES,
            "kick_members" => Permissions::KICK_MEMBERS,
            "ban_members" => Permissions::BAN_MEMBERS,
            "moderate_members" => Permissions::MODERATE_MEMBERS,
            unknown => bail!("unknown Discord permission in Lua manifest: {unknown}"),
        };
        Ok(permissions | permission)
    })
}

fn required_guild(interaction: &CommandInteraction) -> Result<GuildId> {
    interaction.guild_id.context("ta komenda działa tylko na serwerze")
}

fn parse_user_id(value: &str) -> Result<UserId> {
    Ok(UserId::new(
        value.parse().context("Lua returned an invalid Discord user ID")?,
    ))
}

async fn songbird_manager(context: &Context) -> Result<Arc<songbird::Songbird>> {
    songbird::get(context)
        .await
        .context("Songbird voice client is not registered")
}

async fn active_call(
    context: &Context,
    guild_id: GuildId,
) -> Result<Arc<tokio::sync::Mutex<songbird::Call>>> {
    songbird_manager(context)
        .await?
        .get(guild_id)
        .context("bot nie jest połączony z kanałem głosowym")
}

async fn reply(
    context: &Context,
    interaction: &CommandInteraction,
    content: &str,
    ephemeral: bool,
) -> Result<()> {
    interaction
        .create_response(
            &context.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(truncate(content, 2_000))
                    .ephemeral(ephemeral),
            ),
        )
        .await?;
    Ok(())
}

fn truncate(value: &str, maximum: usize) -> String {
    if value.chars().count() <= maximum {
        return value.to_owned();
    }
    value.chars().take(maximum.saturating_sub(1)).collect::<String>() + "…"
}

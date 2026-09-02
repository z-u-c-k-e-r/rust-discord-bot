use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result};
use serenity::{
    async_trait,
    builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage},
    model::{
        application::{Command, Interaction},
        gateway::Ready,
        id::GuildId,
    },
    prelude::{Client, Context, EventHandler, GatewayIntents},
};
use songbird::SerenityInit;
use tokio::sync::Mutex;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use zuckerbot_config::BotConfig;
use zuckerbot_core::{CommandContext, CommandResponse};
use zuckerbot_lua::{LuaLimits, LuaRuntime};

struct Handler {
    runtime: Arc<Mutex<LuaRuntime>>,
    development_guild_id: Option<u64>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        let command_context = CommandContext {
            command_name: command.data.name.clone(),
            interaction_id: command.id.get().to_string(),
            user_id: command.user.id.get().to_string(),
            user_name: command.user.name.clone(),
            guild_id: command.guild_id.map(|guild_id| guild_id.get().to_string()),
            channel_id: command.channel_id.get().to_string(),
            locale: command.locale.clone(),
            options: serde_json::to_value(&command.data.options).unwrap_or(serde_json::Value::Null),
        };

        let response = {
            let runtime = self.runtime.lock().await;
            runtime.execute(&command.data.name, &command_context)
        };

        let response = match response {
            Ok(response) => response,
            Err(reason) => {
                error!(
                    command = %command.data.name,
                    interaction_id = %command.id,
                    error = %reason,
                    "Lua command failed"
                );
                CommandResponse {
                    content: format!(
                        "Nie udało się wykonać komendy. Identyfikator zdarzenia: `{}`.",
                        command.id
                    ),
                    ephemeral: true,
                }
            }
        };

        let message = CreateInteractionResponseMessage::new()
            .content(response.content)
            .ephemeral(response.ephemeral);
        let builder = CreateInteractionResponse::Message(message);

        if let Err(reason) = command.create_response(&ctx.http, builder).await {
            error!(
                command = %command.data.name,
                interaction_id = %command.id,
                error = %reason,
                "Could not send interaction response"
            );
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!(user = %ready.user.name, "Discord gateway connected");

        let command_builders = {
            let runtime = self.runtime.lock().await;
            runtime
                .command_specs()
                .iter()
                .cloned()
                .map(|command| {
                    CreateCommand::new(command.name)
                        .description(command.description)
                        .dm_permission(command.dm_permission)
                })
                .collect::<Vec<_>>()
        };

        let registration_result = match self.development_guild_id {
            Some(guild_id) => {
                info!(guild_id, "Registering development guild commands");
                GuildId::new(guild_id)
                    .set_commands(&ctx.http, command_builders)
                    .await
            }
            None => {
                info!("Registering global commands");
                Command::set_global_commands(&ctx.http, command_builders).await
            }
        };

        match registration_result {
            Ok(commands) => info!(count = commands.len(), "Discord commands synchronized"),
            Err(reason) => error!(error = %reason, "Discord command synchronization failed"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = BotConfig::from_env().context("could not load bot configuration")?;

    let runtime = LuaRuntime::from_directory(
        &config.plugin_directory,
        LuaLimits {
            memory_bytes: config.lua_memory_limit_bytes,
            instructions_per_call: config.lua_instruction_limit,
        },
    )
    .with_context(|| {
        format!(
            "could not load Lua plugins from {}",
            config.plugin_directory.display()
        )
    })?;

    info!(
        plugins = runtime.plugin_metadata().len(),
        commands = runtime.command_specs().len(),
        "Lua plugins loaded"
    );

    let runtime = Arc::new(Mutex::new(runtime));
    let handler = Handler {
        runtime,
        development_guild_id: config.development_guild_id,
    };

    let mut client = Client::builder(config.discord_token(), gateway_intents(&config))
        .event_handler(handler)
        .register_songbird()
        .await
        .context("could not create Discord client")?;

    client
        .start_autosharded()
        .await
        .context("Discord client stopped unexpectedly")
}

fn gateway_intents(config: &BotConfig) -> GatewayIntents {
    let mut intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;

    if config.enable_message_content {
        intents |= GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    }
    if config.enable_guild_members {
        intents |= GatewayIntents::GUILD_MEMBERS;
    }
    if config.enable_guild_presences {
        intents |= GatewayIntents::GUILD_PRESENCES;
    }

    intents
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();
}

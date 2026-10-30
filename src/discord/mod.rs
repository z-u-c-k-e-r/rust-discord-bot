mod commands;
mod executor;
mod handler;
pub mod moderation_cases;
pub mod music;
mod progression;
mod scheduler;
mod tickets;

use std::sync::Arc;

use anyhow::Result;
use serenity::{
    Client,
    all::{ApplicationId, GatewayIntents},
};
use songbird::SerenityInit;

use crate::AppState;

pub async fn run(state: AppState) -> Result<()> {
    let (_tx, rx) = tokio::sync::broadcast::channel(1);
    run_with_shutdown(state, rx).await
}

pub async fn run_with_shutdown(
    state: AppState,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    if state.config.discord_token.starts_with("dummy")
        || state.config.discord_token == "mock"
        || state.config.discord_token == "offline"
        || std::env::var("OFFLINE_MODE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
    {
        tracing::warn!("offline/mock test mode active: skipping live Discord Gateway connection");
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(86400 * 365)) => {}
            _ = shutdown_rx.recv() => {
                tracing::info!("offline discord worker received shutdown signal");
            }
        }
        return Ok(());
    }

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MODERATION
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::MESSAGE_CONTENT;

    let handler = handler::Handler::new(state.clone());
    let mut client = Client::builder(&state.config.discord_token, intents)
        .application_id(ApplicationId::new(state.config.discord_application_id))
        .event_handler(handler)
        .register_songbird()
        .await?;

    let scheduler_worker = tokio::spawn(crate::scheduler::run_worker(
        Arc::clone(&client.http),
        state.clone(),
    ));

    let shard_manager = Arc::clone(&client.shard_manager);
    tokio::spawn(async move {
        let _ = shutdown_rx.recv().await;
        tracing::info!("discord client received shutdown signal, shutting down shards");
        shard_manager.shutdown_all().await;
    });

    let client_result = client.start_autosharded().await;
    scheduler_worker.abort();
    client_result?;
    Ok(())
}

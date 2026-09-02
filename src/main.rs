use std::sync::Arc;

use anyhow::{Context, Result};
use serenity::Client;
use songbird::SerenityInit;
use tracing_subscriber::EnvFilter;
use zuckerbot::{
    config::Config,
    discord::DiscordHandler,
    lua::LuaEngine,
    storage::Storage,
    web::{self, WebState},
};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Arc::new(Config::from_env()?);
    let engine = LuaEngine::load(&config.scripts_dir, config.lua_limits)?;
    let storage = Storage::open(&config.data_dir).await?;

    let handler = DiscordHandler::new(
        Arc::clone(&config),
        engine.clone(),
        storage.clone(),
        reqwest::Client::new(),
    );

    let mut client = Client::builder(&config.discord_token, config.gateway_intents())
        .event_handler(handler)
        .register_songbird()
        .await
        .context("failed to construct Discord client")?;

    let web_state = WebState::new(Arc::clone(&config), engine, storage);
    let web_bind = config.web_bind;
    let dashboard = tokio::spawn(async move { web::serve(web_bind, web_state).await });

    tracing::info!(address = %web_bind, "dashboard server started");

    tokio::select! {
        result = client.start() => {
            result.context("Discord gateway stopped with an error")?;
        }
        result = dashboard => {
            result.context("dashboard task panicked")??;
        }
        signal = tokio::signal::ctrl_c() => {
            signal.context("failed to listen for shutdown signal")?;
            tracing::info!("shutdown signal received");
        }
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("zuckerbot=info,serenity=info,songbird=info,tower_http=info")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();
}

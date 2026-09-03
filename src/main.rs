use anyhow::Result;
use tracing_subscriber::EnvFilter;
use zuckerbot::{AppState, config::Config};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("zuckerbot=info,serenity=info,songbird=info")),
        )
        .init();

    let config = Config::from_env()?;
    let state = AppState::bootstrap(config).await?;

    tokio::try_join!(
        zuckerbot::web::serve(state.clone()),
        zuckerbot::discord::run(state),
    )?;

    Ok(())
}

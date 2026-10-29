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

    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let shutdown_signal_tx = shutdown_tx.clone();

    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_signal_tx.send(());
    });

    let web_shutdown_rx = shutdown_tx.subscribe();
    let discord_shutdown_rx = shutdown_tx.subscribe();

    tokio::select! {
        res = zuckerbot::web::serve_with_shutdown(state.clone(), web_shutdown_rx) => {
            if let Err(e) = res {
                tracing::error!(?e, "web dashboard exited with error");
            }
        }
        res = zuckerbot::discord::run_with_shutdown(state, discord_shutdown_rx) => {
            if let Err(e) = res {
                tracing::error!(?e, "discord gateway client exited with error");
            }
        }
    }

    tracing::info!("zuckerbot graceful shutdown complete");
    Ok(())
}

async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => signal.recv().await,
            Err(e) => {
                tracing::error!(?e, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
                None
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received SIGINT (Ctrl+C), initiating graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("received SIGTERM, initiating graceful shutdown");
        }
    }
}

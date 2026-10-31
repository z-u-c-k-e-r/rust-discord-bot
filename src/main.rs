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
    let state_for_signal = state.clone();

    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        state_for_signal
            .is_shutting_down
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = shutdown_signal_tx.send(());
    });

    let web_shutdown_rx = shutdown_tx.subscribe();
    let discord_shutdown_rx = shutdown_tx.subscribe();

    let web_handle = tokio::spawn(zuckerbot::web::serve_with_shutdown(
        state.clone(),
        web_shutdown_rx,
    ));
    let discord_handle = tokio::spawn(zuckerbot::discord::run_with_shutdown(
        state.clone(),
        discord_shutdown_rx,
    ));

    tokio::pin!(web_handle);
    tokio::pin!(discord_handle);

    let result = tokio::select! {
        res = &mut web_handle => {
            match res {
                Ok(Ok(())) => {
                    tracing::info!("web service stopped cleanly");
                    Ok(())
                }
                Ok(Err(e)) => {
                    tracing::error!(?e, "web service failed with error");
                    Err(e)
                }
                Err(join_err) => {
                    tracing::error!(?join_err, "web task panicked");
                    Err(anyhow::anyhow!("web task panicked: {join_err}"))
                }
            }
        }
        res = &mut discord_handle => {
            match res {
                Ok(Ok(())) => {
                    tracing::info!("discord service stopped cleanly");
                    Ok(())
                }
                Ok(Err(e)) => {
                    tracing::error!(?e, "discord service failed with error");
                    Err(e)
                }
                Err(join_err) => {
                    tracing::error!(?join_err, "discord task panicked");
                    Err(anyhow::anyhow!("discord task panicked: {join_err}"))
                }
            }
        }
    };

    // If one service exited, initiate coordinated shutdown of the other
    state
        .is_shutting_down
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = shutdown_tx.send(());

    // Wait for the remaining service with bounded timeout
    let remaining_timeout = std::time::Duration::from_secs(10);
    let shutdown_drain = async {
        if !web_handle.is_finished() {
            let _ = (&mut web_handle).await;
        }
        if !discord_handle.is_finished() {
            let _ = (&mut discord_handle).await;
        }
    };

    if tokio::time::timeout(remaining_timeout, shutdown_drain)
        .await
        .is_err()
    {
        tracing::warn!("timed out waiting for background services to terminate during shutdown");
    }

    tracing::info!("zuckerbot graceful shutdown complete");
    result
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

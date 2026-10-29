mod auth;
mod routes;
mod static_files;

use std::sync::Arc;

use anyhow::Result;
use axum::{
    Router,
    routing::{get, post, put},
};
use dashmap::DashMap;
use tokio::net::TcpListener;

use crate::AppState;

#[derive(Clone)]
pub struct WebState {
    pub app: AppState,
    pub sessions: Arc<DashMap<String, auth::Session>>,
    pub oauth_states: Arc<DashMap<String, chrono::DateTime<chrono::Utc>>>,
    pub login_rate_limiter: Arc<auth::LoginRateLimiter>,
    pub permissions_cache: Arc<DashMap<(String, String), auth::CachedGuildPermission>>,
}

pub async fn serve(app: AppState) -> Result<()> {
    let (_tx, rx) = tokio::sync::broadcast::channel(1);
    serve_with_shutdown(app, rx).await
}

pub async fn serve_with_shutdown(
    app: AppState,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    let bind = app.config.dashboard_bind;
    let state = WebState {
        app,
        sessions: Arc::new(DashMap::new()),
        oauth_states: Arc::new(DashMap::new()),
        login_rate_limiter: Arc::new(auth::LoginRateLimiter::new()),
        permissions_cache: Arc::new(DashMap::new()),
    };

    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let (states, sessions, perms) = auth::cleanup_stale_records(&cleanup_state);
            if states > 0 || sessions > 0 || perms > 0 {
                tracing::debug!(
                    states,
                    sessions,
                    perms,
                    "pruned stale OAuth states, expired sessions, and permissions"
                );
            }
        }
    });

    let router = Router::new()
        .route("/", get(static_files::index))
        .route("/assets/styles.css", get(static_files::styles))
        .route("/assets/app.js", get(static_files::javascript))
        .route("/healthz", get(routes::health_live))
        .route("/health/live", get(routes::health_live))
        .route("/readyz", get(routes::health_ready))
        .route("/health/ready", get(routes::health_ready))
        .route("/metrics", get(routes::prometheus_metrics))
        .route("/auth/discord", get(auth::login))
        .route("/auth/discord/callback", get(auth::callback))
        .route("/api/session", get(auth::current_session))
        .route("/api/logout", post(auth::logout))
        .route("/api/modules", get(routes::list_modules))
        .route("/api/guilds/{guild_id}/modules", get(routes::guild_modules))
        .route(
            "/api/guilds/{guild_id}/modules/{module_id}",
            put(routes::update_guild_module),
        )
        .with_state(state);

    let listener = TcpListener::bind(bind).await?;
    tracing::info!(%bind, "dashboard is listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            tracing::info!("dashboard received shutdown signal");
        })
        .await?;

    Ok(())
}

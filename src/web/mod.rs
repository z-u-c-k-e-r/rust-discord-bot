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
    app: AppState,
    sessions: Arc<DashMap<String, auth::Session>>,
    oauth_states: Arc<DashMap<String, chrono::DateTime<chrono::Utc>>>,
}

pub async fn serve(app: AppState) -> Result<()> {
    let bind = app.config.dashboard_bind;
    let state = WebState {
        app,
        sessions: Arc::new(DashMap::new()),
        oauth_states: Arc::new(DashMap::new()),
    };

    let router = Router::new()
        .route("/", get(static_files::index))
        .route("/assets/styles.css", get(static_files::styles))
        .route("/assets/app.js", get(static_files::javascript))
        .route("/healthz", get(routes::health))
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
    axum::serve(listener, router).await?;

    Ok(())
}

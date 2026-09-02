use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::Serialize;
use tower_http::trace::TraceLayer;

use crate::{
    config::Config,
    lua::LuaEngine,
    model::{GuildConfig, ModuleSummary},
    storage::Storage,
};

#[derive(Clone)]
pub struct WebState {
    config: Arc<Config>,
    engine: LuaEngine,
    storage: Storage,
}

impl WebState {
    pub fn new(config: Arc<Config>, engine: LuaEngine, storage: Storage) -> Self {
        Self {
            config,
            engine,
            storage,
        }
    }
}

pub async fn serve(address: SocketAddr, state: WebState) -> Result<()> {
    let app = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/modules", get(modules))
        .route("/api/reload", post(reload_modules))
        .route(
            "/api/guilds/{guild_id}/config",
            get(get_guild_config).put(put_guild_config),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind dashboard to {address}"))?;
    axum::serve(listener, app)
        .await
        .context("dashboard HTTP server stopped")
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        service: "zuckerbot",
    })
}

async fn modules(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<ModuleSummary>>> {
    authorize(&headers, &state.config.dashboard_token)?;
    Ok(Json(state.engine.module_summaries()))
}

async fn reload_modules(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> ApiResult<Json<ReloadResponse>> {
    authorize(&headers, &state.config.dashboard_token)?;
    let count = state.engine.reload().map_err(ApiError::internal)?;
    Ok(Json(ReloadResponse { loaded_modules: count }))
}

#[derive(Serialize)]
struct ReloadResponse {
    loaded_modules: usize,
}

async fn get_guild_config(
    State(state): State<WebState>,
    Path(guild_id): Path<u64>,
    headers: HeaderMap,
) -> ApiResult<Json<GuildConfig>> {
    authorize(&headers, &state.config.dashboard_token)?;
    let config = state
        .storage
        .guild_config(guild_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(config))
}

async fn put_guild_config(
    State(state): State<WebState>,
    Path(guild_id): Path<u64>,
    headers: HeaderMap,
    Json(config): Json<GuildConfig>,
) -> ApiResult<StatusCode> {
    authorize(&headers, &state.config.dashboard_token)?;

    if let Some(enabled) = &config.enabled_modules {
        let installed = state
            .engine
            .module_summaries()
            .into_iter()
            .map(|module| module.name)
            .collect::<std::collections::HashSet<_>>();
        if let Some(unknown) = enabled.iter().find(|module| !installed.contains(*module)) {
            return Err(ApiError::bad_request(format!(
                "unknown Lua module: {unknown}"
            )));
        }
    }

    state
        .storage
        .save_guild_config(guild_id, config)
        .await
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

fn authorize(headers: &HeaderMap, expected: &str) -> ApiResult<()> {
    let supplied = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;

    if !constant_time_eq(supplied.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::unauthorized("invalid bearer token"));
    }
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

type ApiResult<T> = std::result::Result<T, ApiError>;

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "dashboard request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(serde_json::json!({ "error": self.message }))).into_response()
    }
}

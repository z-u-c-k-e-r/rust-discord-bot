use std::{sync::Arc, time::Instant};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    response::{Html, IntoResponse},
    routing::get,
};
use serde::Serialize;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;
use zuckerbot_config::ApiConfig;

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLES_CSS: &str = include_str!("../static/styles.css");

#[derive(Clone)]
struct AppState {
    started_at: Instant,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct MetaResponse {
    product: &'static str,
    phase: &'static str,
    architecture: Vec<&'static str>,
    modules: Vec<ModuleSummary>,
}

#[derive(Serialize)]
struct ModuleSummary {
    id: &'static str,
    name: &'static str,
    status: &'static str,
    description: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let config = ApiConfig::from_env().context("could not load API configuration")?;
    let state = Arc::new(AppState {
        started_at: Instant::now(),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/assets/app.js", get(javascript))
        .route("/assets/styles.css", get(stylesheet))
        .route("/health", get(health))
        .route("/api/v1/meta", get(meta))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("could not bind API to {}", config.bind_address))?;

    info!(address = %config.bind_address, "ZuckerBot control API started");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("control API stopped unexpectedly")
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn javascript() -> impl IntoResponse {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        APP_JS,
    )
}

async fn stylesheet() -> impl IntoResponse {
    ([("content-type", "text/css; charset=utf-8")], STYLES_CSS)
}

async fn health(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "zuckerbot-api",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

async fn meta() -> Json<MetaResponse> {
    Json(MetaResponse {
        product: "ZuckerBot",
        phase: "platform-foundation",
        architecture: vec![
            "Rust control plane and Discord gateway",
            "Sandboxed Lua 5.4 extension runtime",
            "PostgreSQL durable storage",
            "Redis cache, jobs and distributed coordination",
            "Web dashboard with Discord OAuth2 and guild RBAC",
        ],
        modules: vec![
            ModuleSummary {
                id: "lua",
                name: "Lua Runtime",
                status: "available",
                description: "Command discovery, validation and bounded execution.",
            },
            ModuleSummary {
                id: "gateway",
                name: "Discord Gateway",
                status: "available",
                description: "Slash-command synchronization and interaction dispatch.",
            },
            ModuleSummary {
                id: "voice",
                name: "Voice Foundation",
                status: "foundation",
                description: "Songbird voice manager registered; queue and providers follow.",
            },
            ModuleSummary {
                id: "dashboard",
                name: "Control Dashboard",
                status: "foundation",
                description: "Health and architecture API; OAuth2 configuration follows.",
            },
        ],
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();
}

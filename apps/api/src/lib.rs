use std::{
    borrow::Cow,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{Html, IntoResponse},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use zuckerbot_config::ApiConfig;
use zuckerbot_persistence::{
    ConfigurationValidationError, ControlPlaneStore, GuildModuleConfiguration, PutGuildModule,
    ReadinessReport, StoreError,
};

const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_JS: &str = include_str!("../static/app.js");
const STYLES_CSS: &str = include_str!("../static/styles.css");
const MAX_REQUEST_BODY_BYTES: usize = 128 * 1024;

#[derive(Clone)]
struct AppState {
    started_at: Instant,
    store: ControlPlaneStore,
    development_auth: Option<DevelopmentAuth>,
}

#[derive(Clone)]
struct DevelopmentAuth {
    token_hash: [u8; 32],
    actor_user_id: Arc<str>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: &'static str,
    dependencies: ReadinessReport,
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

#[derive(Debug, Deserialize)]
struct PutModuleRequest {
    enabled: bool,
    configuration: Value,
    #[serde(default)]
    expected_version: Option<i64>,
}

#[derive(Serialize)]
struct ModuleWriteResponse {
    request_id: String,
    module: GuildModuleConfiguration,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: Cow<'static, str>,
    details: Option<Value>,
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a Value>,
}

pub async fn run() -> Result<()> {
    init_tracing();
    let config = ApiConfig::from_env().context("could not load API configuration")?;
    let store = ControlPlaneStore::new(
        &config.database_url,
        &config.redis_url,
        config.postgres_max_connections,
        config.dependency_timeout,
        config.migration_timeout,
    )
    .context("could not initialize control-plane dependencies")?;

    if config.run_migrations {
        store
            .migrate()
            .await
            .context("could not apply database migrations")?;
    }

    let development_auth = match (
        config.control_plane_dev_token(),
        config.control_plane_dev_actor_id.as_deref(),
    ) {
        (Some(token), Some(actor_user_id)) => {
            info!(
                actor_user_id,
                "development control-plane authentication is enabled"
            );
            Some(DevelopmentAuth::new(token, actor_user_id))
        }
        _ => None,
    };
    let state = Arc::new(AppState {
        started_at: Instant::now(),
        store,
        development_auth,
    });
    let app = build_router(state);

    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("could not bind API to {}", config.bind_address))?;
    info!(address = %config.bind_address, "ZuckerBot control API started");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("control API stopped unexpectedly")
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/app.js", get(javascript))
        .route("/assets/styles.css", get(stylesheet))
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/v1/meta", get(meta))
        .route(
            "/api/v1/guilds/{guild_id}/modules/{module_id}",
            get(get_module).put(put_module),
        )
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
        .layer(TimeoutLayer::new(Duration::from_secs(15)))
        .layer(TraceLayer::new_for_http())
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

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "zuckerbot-api",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

async fn ready(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let dependencies = state.store.readiness().await;
    let (status_code, status) = if dependencies.ready {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not_ready")
    };
    (
        status_code,
        Json(ReadinessResponse {
            status,
            dependencies,
        }),
    )
}

async fn meta() -> Json<MetaResponse> {
    Json(MetaResponse {
        product: "ZuckerBot",
        phase: "control-plane-foundation",
        architecture: vec![
            "Rust control plane and Discord gateway",
            "Sandboxed Lua 5.4 extension runtime",
            "PostgreSQL durable configuration and audit",
            "Redis ephemeral state and distributed coordination",
            "Web dashboard with Discord OAuth2 and guild RBAC",
        ],
        modules: vec![
            ModuleSummary {
                id: "persistence",
                name: "Persistent Control Plane",
                status: "available",
                description: "Versioned module settings, transactions and audit events.",
            },
            ModuleSummary {
                id: "readiness",
                name: "Dependency Readiness",
                status: "available",
                description: "Independent PostgreSQL and Redis health reporting.",
            },
            ModuleSummary {
                id: "oauth",
                name: "Discord OAuth2",
                status: "in_progress",
                description: "Server-side sessions and guild permission checks follow next.",
            },
            ModuleSummary {
                id: "dashboard",
                name: "Control Dashboard",
                status: "foundation",
                description: "The authenticated configuration interface follows OAuth2.",
            },
        ],
    })
}

async fn get_module(
    State(state): State<Arc<AppState>>,
    Path((guild_id, module_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<GuildModuleConfiguration>, ApiError> {
    state.authorize_development(&headers)?;
    let module = state
        .store
        .get_guild_module(&guild_id, &module_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| ApiError::not_found("module_configuration_not_found"))?;
    Ok(Json(module))
}

async fn put_module(
    State(state): State<Arc<AppState>>,
    Path((guild_id, module_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<PutModuleRequest>,
) -> Result<(StatusCode, Json<ModuleWriteResponse>), ApiError> {
    let actor_user_id = state.authorize_development(&headers)?;
    let request_id = Uuid::new_v4().to_string();
    let module = state
        .store
        .put_guild_module(PutGuildModule {
            guild_id,
            module_id,
            enabled: request.enabled,
            configuration: request.configuration,
            expected_version: request.expected_version,
            actor_user_id,
            request_id: request_id.clone(),
        })
        .await
        .map_err(map_store_error)?;
    let status = if module.version == 1 {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(ModuleWriteResponse { request_id, module })))
}

impl DevelopmentAuth {
    fn new(token: &str, actor_user_id: &str) -> Self {
        Self {
            token_hash: Sha256::digest(token.as_bytes()).into(),
            actor_user_id: Arc::from(actor_user_id),
        }
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<String, ApiError> {
        let supplied_token = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or_else(ApiError::unauthorized)?;
        let supplied_hash: [u8; 32] = Sha256::digest(supplied_token.as_bytes()).into();

        if bool::from(self.token_hash.ct_eq(&supplied_hash)) {
            Ok(self.actor_user_id.to_string())
        } else {
            Err(ApiError::unauthorized())
        }
    }
}

impl AppState {
    fn authorize_development(&self, headers: &HeaderMap) -> Result<String, ApiError> {
        self.development_auth
            .as_ref()
            .ok_or_else(|| {
                ApiError::service_unavailable(
                    "authentication_not_configured",
                    "Control-plane writes are disabled until Discord OAuth2 or development authentication is configured.",
                )
            })?
            .authorize(headers)
    }
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: None,
        }
    }

    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "A valid control-plane session is required.",
        )
    }

    fn not_found(code: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            code,
            "The requested resource does not exist.",
        )
    }

    fn service_unavailable(code: &'static str, message: &'static str) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: &self.message,
                details: self.details.as_ref(),
            },
        };
        (self.status, Json(body)).into_response()
    }
}

fn map_store_error(error: StoreError) -> ApiError {
    match error {
        StoreError::Validation(validation) => ApiError::new(
            StatusCode::BAD_REQUEST,
            validation_code(&validation),
            validation.to_string(),
        ),
        StoreError::VersionConflict { current_version } => ApiError::new(
            StatusCode::CONFLICT,
            "module_version_conflict",
            "The module configuration changed after it was loaded. Reload it and retry.",
        )
        .with_details(serde_json::json!({ "current_version": current_version })),
        internal_error => {
            error!(error = %internal_error, "control-plane persistence operation failed");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The control plane could not complete the operation.",
            )
        }
    }
}

fn validation_code(error: &ConfigurationValidationError) -> &'static str {
    match error {
        ConfigurationValidationError::InvalidGuildId => "invalid_guild_id",
        ConfigurationValidationError::InvalidActorUserId => "invalid_actor_user_id",
        ConfigurationValidationError::InvalidModuleId => "invalid_module_id",
        ConfigurationValidationError::ConfigurationMustBeObject => "configuration_must_be_object",
        ConfigurationValidationError::ConfigurationTooLarge => "configuration_too_large",
        ConfigurationValidationError::InvalidExpectedVersion => "invalid_expected_version",
        ConfigurationValidationError::InvalidRequestId => "invalid_request_id",
    }
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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn development_auth_accepts_only_the_configured_token() {
        let auth = DevelopmentAuth::new(
            "0123456789012345678901234567890123456789",
            "123456789012345678",
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789012345678901234567890123456789"),
        );
        assert_eq!(auth.authorize(&headers).unwrap(), "123456789012345678");

        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer wrong"));
        assert!(auth.authorize(&headers).is_err());
    }

    #[test]
    fn store_validation_errors_have_stable_api_codes() {
        assert_eq!(
            validation_code(&ConfigurationValidationError::InvalidModuleId),
            "invalid_module_id"
        );
        assert_eq!(
            validation_code(&ConfigurationValidationError::ConfigurationTooLarge),
            "configuration_too_large"
        );
    }
}

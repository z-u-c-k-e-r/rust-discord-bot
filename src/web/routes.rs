use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::lua::LuaModuleManifest;

use super::{
    WebState,
    auth::{authenticate, ensure_guild_access, verify_csrf},
};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    pub details: Option<Vec<String>>,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            details: None,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            details: None,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
            details: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            details: None,
        }
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
            details: None,
        }
    }

    pub fn validation(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: message.into(),
            details: Some(details),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let message = message.into();
        tracing::error!(%message, "dashboard internal error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Wewnętrzny błąd panelu.".to_owned(),
            details: None,
        }
    }

    pub fn upstream(error: reqwest::Error) -> Self {
        tracing::warn!(?error, "Discord OAuth2 request failed");
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: "Discord nie odpowiedział poprawnie podczas logowania.".to_owned(),
            details: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.message });
        if let Some(details) = self.details {
            body["details"] = json!(details);
        }
        (self.status, Json(body)).into_response()
    }
}

#[derive(Serialize)]
pub struct ModuleView {
    manifest: LuaModuleManifest,
    enabled: bool,
    config: Value,
    configured: bool,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct UpdateModuleRequest {
    enabled: bool,
    #[serde(default)]
    config: Value,
}

pub async fn health_live(State(state): State<WebState>) -> Json<Value> {
    state
        .app
        .metrics
        .dashboard_requests_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.app.metrics.uptime_seconds(),
    }))
}

pub async fn health_ready(
    State(state): State<WebState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    state
        .app
        .metrics
        .dashboard_requests_total
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut checks = serde_json::Map::new();
    let mut is_ready = true;

    match state.app.storage.ping().await {
        Ok(()) => {
            checks.insert("storage".to_owned(), json!("ok"));
        }
        Err(e) => {
            is_ready = false;
            checks.insert("storage".to_owned(), json!(format!("error: {e}")));
        }
    }

    let manifest_count = state.app.scripts.manifests().await.len();
    if manifest_count > 0 {
        checks.insert(
            "lua_engine".to_owned(),
            json!(format!("ok ({manifest_count} modules)")),
        );
    } else {
        is_ready = false;
        checks.insert("lua_engine".to_owned(), json!("error: no modules loaded"));
    }

    let payload = json!({
        "status": if is_ready { "ready" } else { "unready" },
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.app.metrics.uptime_seconds(),
        "checks": checks,
    });

    if is_ready {
        Ok(Json(payload))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(payload)))
    }
}

pub async fn prometheus_metrics(State(state): State<WebState>) -> impl IntoResponse {
    use axum::http::header::CONTENT_TYPE;
    let active_sessions = state
        .app
        .sessions
        .count_active_sessions()
        .await
        .unwrap_or(0);
    let body = state.app.metrics.render_prometheus(active_sessions);
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

pub async fn list_modules(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LuaModuleManifest>>, ApiError> {
    authenticate(&state, &headers).await?;
    Ok(Json(state.app.scripts.manifests().await))
}

pub async fn guild_modules(
    State(state): State<WebState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ModuleView>>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;

    let manifests = state.app.scripts.manifests().await;
    let mut modules = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let settings = state
            .app
            .storage
            .get_module_settings(&guild_id, &manifest.id)
            .await
            .map_err(|error| ApiError::internal(error.to_string()))?;

        modules.push(match settings {
            Some(settings) => ModuleView {
                manifest,
                enabled: settings.enabled,
                config: settings.config,
                configured: true,
                updated_at: Some(settings.updated_at),
            },
            None => ModuleView {
                enabled: manifest.default_enabled,
                manifest,
                config: json!({}),
                configured: false,
                updated_at: None,
            },
        });
    }

    Ok(Json(modules))
}

pub async fn update_guild_module(
    State(state): State<WebState>,
    Path((guild_id, module_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateModuleRequest>,
) -> Result<Json<ModuleView>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    verify_csrf(&headers, &session)?;

    let decision = state.app.rate_limiter.check_action(
        "admin:update_module",
        &session.user.id,
        Some(&guild_id),
    );
    if !decision.allowed {
        state
            .app
            .metrics
            .rate_limit_throttled_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Err(ApiError::too_many_requests(format!(
            "Zbyt częste zmiany konfiguracji modułów. Odczekaj {} s.",
            decision.reset_after.as_secs().max(1)
        )));
    }

    let manifest = state
        .app
        .scripts
        .manifest(&module_id)
        .await
        .ok_or_else(|| ApiError::not_found("Nie znaleziono modułu Lua."))?;

    if !payload.config.is_object() {
        return Err(ApiError::bad_request(
            "Konfiguracja modułu musi być obiektem JSON.",
        ));
    }
    let serialized_size = serde_json::to_vec(&payload.config)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .len();
    if serialized_size > 64 * 1024 {
        return Err(ApiError::bad_request(
            "Konfiguracja modułu nie może przekraczać 64 KiB.",
        ));
    }

    if let Err(validation_errors) = state
        .app
        .scripts
        .validate_module_config(&module_id, &payload.config)
        .await
    {
        return Err(ApiError::validation(
            "Konfiguracja modułu jest niezgodna ze schematem JSON.",
            validation_errors,
        ));
    }

    let settings = state
        .app
        .storage
        .update_module_settings_with_audit(crate::storage::UpdateModuleSettingsParams {
            guild_id: &guild_id,
            module_id: &module_id,
            enabled: payload.enabled,
            config: payload.config,
            actor_id: Some(&session.user.id),
            audit_event: "module_settings_updated",
            audit_data: json!({
                "module_id": module_id,
                "enabled": payload.enabled,
            }),
        })
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(ModuleView {
        manifest,
        enabled: settings.enabled,
        config: settings.config,
        configured: true,
        updated_at: Some(settings.updated_at),
    }))
}

pub async fn guild_cases(
    State(state): State<WebState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::moderation_cases::ModerationCase>>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    let guild_u64 = guild_id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator serwera."))?;
    let cases = state
        .app
        .moderation_cases
        .list_cases(
            guild_u64,
            crate::moderation_cases::CaseFilter {
                subject_user_id: None,
                actor_user_id: None,
                kind: None,
                status: None,
                visible_to_subject_only: false,
                limit: 50,
            },
            Utc::now(),
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(cases))
}

pub async fn guild_case_details(
    State(state): State<WebState>,
    Path((guild_id, case_number)): Path<(String, i64)>,
    headers: HeaderMap,
) -> Result<Json<crate::moderation_cases::CaseDetails>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    let guild_u64 = guild_id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator serwera."))?;
    let details = state
        .app
        .moderation_cases
        .get_case(guild_u64, case_number, Utc::now())
        .await
        .map_err(|error| {
            ApiError::not_found(format!("Nie znaleziono sprawy #{case_number}: {error}"))
        })?;

    Ok(Json(details))
}

pub async fn guild_audit_events(
    State(state): State<WebState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::storage::AuditLogEntry>>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    let events = state
        .app
        .storage
        .list_audit_events(&guild_id, 100)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(events))
}

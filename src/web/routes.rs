use std::convert::Infallible;
use std::sync::atomic::Ordering;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

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

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
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
        tracing::warn!(?error, "Discord OAuth2/API request failed");
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: "Discord nie odpowiedział poprawnie podczas komunikacji z API.".to_owned(),
            details: None,
        }
    }
}

impl From<crate::moderation_cases::CaseError> for ApiError {
    fn from(error: crate::moderation_cases::CaseError) -> Self {
        match error {
            crate::moderation_cases::CaseError::NotFound => {
                Self::not_found("Sprawa moderacyjna nie została znaleziona.")
            }
            crate::moderation_cases::CaseError::VersionConflict { expected, actual } => {
                Self::conflict(format!(
                    "Konflikt wersji sprawy: oczekiwano {expected}, aktualna wersja to {actual}. Odśwież widok."
                ))
            }
            crate::moderation_cases::CaseError::Validation(msg) => Self::bad_request(msg),
            crate::moderation_cases::CaseError::Database(err) => {
                Self::internal(format!("Błąd bazy danych sprawy: {err}"))
            }
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
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.app.metrics.uptime_seconds(),
    }))
}

pub async fn health_ready(
    State(state): State<WebState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
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

    if state.app.is_shutting_down.load(Ordering::SeqCst) {
        is_ready = false;
        checks.insert(
            "lifecycle".to_owned(),
            json!("unready (process is shutting down)"),
        );
    } else {
        checks.insert("lifecycle".to_owned(), json!("ok (running)"));
    }

    let is_offline_mode = state.app.config.discord_token.starts_with("dummy")
        || state.app.config.discord_token.starts_with("mock");
    if is_offline_mode {
        checks.insert(
            "discord_gateway".to_owned(),
            json!("ok (offline/dummy mode)"),
        );
    } else if state.app.is_gateway_ready.load(Ordering::SeqCst) {
        checks.insert("discord_gateway".to_owned(), json!("ok (gateway ready)"));
    } else {
        is_ready = false;
        checks.insert(
            "discord_gateway".to_owned(),
            json!("unready (waiting for gateway ready event)"),
        );
    }

    let payload = json!({
        "status": if is_ready { "ready" } else { "unready" },
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_seconds": state.app.metrics.uptime_seconds(),
        "dashboard_requests_total": state.app.metrics.dashboard_requests_total.load(Ordering::Relaxed),
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

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "module_updated".to_string(),
            payload: json!({
                "module_id": module_id,
                "enabled": settings.enabled,
            }),
        });

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
        .map_err(ApiError::from)?;

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

#[derive(Deserialize)]
pub struct AddCaseNotePayload {
    pub body: String,
    #[serde(default)]
    pub visible_to_subject: bool,
}

#[derive(Deserialize)]
pub struct VoidCasePayload {
    pub reason: String,
    #[serde(default)]
    pub expected_version: i64,
}

#[derive(Deserialize)]
pub struct UpdateCasePayload {
    #[serde(default)]
    pub expected_version: i64,
    pub reason: Option<String>,
    pub severity: Option<String>,
    pub points: Option<i32>,
    pub visible_to_subject: Option<bool>,
}

pub async fn add_guild_case_note(
    State(state): State<WebState>,
    Path((guild_id, case_number)): Path<(String, i64)>,
    headers: HeaderMap,
    Json(payload): Json<AddCaseNotePayload>,
) -> Result<Json<crate::moderation_cases::CaseNote>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    verify_csrf(&headers, &session)?;

    let trimmed = payload.body.trim();
    if trimmed.is_empty() || trimmed.chars().count() > crate::moderation_cases::MAX_NOTE_LENGTH {
        return Err(ApiError::bad_request(
            "Treść notatki nie może być pusta ani przekraczać 2000 znaków.",
        ));
    }

    let guild_u64 = guild_id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator serwera."))?;
    let actor_u64 = session
        .user
        .id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator użytkownika."))?;

    let now = Utc::now();
    let note = state
        .app
        .moderation_cases
        .add_note(
            guild_u64,
            case_number,
            actor_u64,
            trimmed.to_owned(),
            payload.visible_to_subject,
            now,
        )
        .await
        .map_err(ApiError::from)?;

    let _ = state
        .app
        .storage
        .record_audit(
            Some(&guild_id),
            Some(&session.user.id),
            "dashboard",
            "case_note_added",
            json!({
                "case_number": case_number,
                "visible_to_subject": payload.visible_to_subject,
            }),
        )
        .await;

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "case_updated".to_string(),
            payload: json!({
                "case_number": case_number,
                "action": "note_added",
            }),
        });

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "audit_entry".to_string(),
            payload: json!({
                "module_id": "dashboard",
                "event": "case_note_added",
            }),
        });

    Ok(Json(note))
}

pub async fn void_guild_case(
    State(state): State<WebState>,
    Path((guild_id, case_number)): Path<(String, i64)>,
    headers: HeaderMap,
    Json(payload): Json<VoidCasePayload>,
) -> Result<Json<crate::moderation_cases::ModerationCase>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    verify_csrf(&headers, &session)?;

    let trimmed = payload.reason.trim();
    if trimmed.is_empty() || trimmed.chars().count() > crate::moderation_cases::MAX_REASON_LENGTH {
        return Err(ApiError::bad_request(
            "Powód unieważnienia nie może być pusty ani przekraczać 2000 znaków.",
        ));
    }

    let guild_u64 = guild_id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator serwera."))?;
    let actor_u64 = session
        .user
        .id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator użytkownika."))?;

    let now = Utc::now();
    if payload.expected_version <= 0 {
        return Err(ApiError::bad_request(
            "Wymagane jest dodatnie expected_version do kontroli wersji.",
        ));
    }
    let version = payload.expected_version;

    let voided = state
        .app
        .moderation_cases
        .void_case(
            guild_u64,
            case_number,
            actor_u64,
            version,
            trimmed.to_owned(),
            now,
        )
        .await
        .map_err(ApiError::from)?;

    let _ = state
        .app
        .storage
        .record_audit(
            Some(&guild_id),
            Some(&session.user.id),
            "dashboard",
            "case_voided",
            json!({
                "case_number": case_number,
                "reason": trimmed,
            }),
        )
        .await;

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "case_updated".to_string(),
            payload: json!({
                "case_number": case_number,
                "action": "voided",
            }),
        });

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "audit_entry".to_string(),
            payload: json!({
                "module_id": "dashboard",
                "event": "case_voided",
            }),
        });

    Ok(Json(voided))
}

pub async fn update_guild_case(
    State(state): State<WebState>,
    Path((guild_id, case_number)): Path<(String, i64)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateCasePayload>,
) -> Result<Json<crate::moderation_cases::ModerationCase>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;
    verify_csrf(&headers, &session)?;

    let guild_u64 = guild_id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator serwera."))?;
    let actor_u64 = session
        .user
        .id
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Niepoprawny identyfikator użytkownika."))?;

    let now = Utc::now();
    if payload.expected_version <= 0 {
        return Err(ApiError::bad_request(
            "Wymagane jest dodatnie expected_version do kontroli wersji.",
        ));
    }
    let version = payload.expected_version;

    let sev = match payload.severity.as_deref() {
        Some(s) => Some(
            s.parse::<crate::moderation_cases::CaseSeverity>()
                .map_err(|e| ApiError::bad_request(e.to_string()))?,
        ),
        None => None,
    };

    let update = crate::moderation_cases::UpdateCase {
        reason: payload.reason,
        severity: sev,
        points: payload.points,
        visible_to_subject: payload.visible_to_subject,
        expires_at: None,
        clear_expiry: false,
    };

    let updated = state
        .app
        .moderation_cases
        .update_case(guild_u64, case_number, actor_u64, version, update, now)
        .await
        .map_err(ApiError::from)?;

    let _ = state
        .app
        .storage
        .record_audit(
            Some(&guild_id),
            Some(&session.user.id),
            "dashboard",
            "case_edited",
            json!({
                "case_number": case_number,
                "version": updated.version,
            }),
        )
        .await;

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "case_updated".to_string(),
            payload: json!({
                "case_number": case_number,
                "version": updated.version,
                "action": "edited",
            }),
        });

    let _ = state
        .app
        .guild_events
        .send(crate::state::GuildRealtimeEvent {
            guild_id: guild_id.clone(),
            event_type: "audit_entry".to_string(),
            payload: json!({
                "module_id": "dashboard",
                "event": "case_edited",
            }),
        });

    Ok(Json(updated))
}

pub async fn guild_events_stream(
    State(state): State<WebState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let (_, session) = authenticate(&state, &headers).await?;
    ensure_guild_access(&state, &session, &guild_id).await?;

    let rx = state.app.guild_events.subscribe();
    let target_guild = guild_id.clone();

    let initial = tokio_stream::once(Ok(Event::default()
        .event("connected")
        .data(json!({ "guild_id": target_guild }).to_string())));

    let stream = BroadcastStream::new(rx).filter_map(move |item| match item {
        Ok(event) if event.guild_id == target_guild => Some(Ok(Event::default()
            .event(event.event_type)
            .data(event.payload.to_string()))),
        _ => None,
    });

    let combined = initial.chain(stream);

    Ok(Sse::new(combined).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

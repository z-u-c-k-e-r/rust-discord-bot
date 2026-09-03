use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{lua::LuaModuleManifest, storage::ModerationCase};

use super::{
    WebState,
    auth::{authenticate, ensure_guild_access, verify_csrf},
};

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let message = message.into();
        tracing::error!(%message, "dashboard internal error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Wewnętrzny błąd panelu.".to_owned(),
        }
    }

    pub fn upstream(error: reqwest::Error) -> Self {
        tracing::warn!(?error, "Discord OAuth2 request failed");
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: "Discord nie odpowiedział poprawnie podczas logowania.".to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

#[derive(Serialize)]
pub struct Health {
    status: &'static str,
    version: &'static str,
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

#[derive(Deserialize)]
pub struct ModerationCasesQuery {
    #[serde(default)]
    include_resolved: bool,
    #[serde(default = "default_moderation_case_limit")]
    limit: u8,
}

#[derive(Deserialize)]
pub struct ResolveModerationCaseRequest {
    resolution: String,
}

pub async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn list_modules(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LuaModuleManifest>>, ApiError> {
    authenticate(&state, &headers)?;
    Ok(Json(state.app.scripts.manifests().await))
}

pub async fn guild_modules(
    State(state): State<WebState>,
    Path(guild_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ModuleView>>, ApiError> {
    let (_, session) = authenticate(&state, &headers)?;
    ensure_guild_access(&session, &guild_id)?;

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
    let (_, session) = authenticate(&state, &headers)?;
    ensure_guild_access(&session, &guild_id)?;
    verify_csrf(&headers, &session)?;

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

    let settings = state
        .app
        .storage
        .set_module_settings(&guild_id, &module_id, payload.enabled, payload.config)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    state
        .app
        .storage
        .record_audit(
            Some(&guild_id),
            Some(&session.user.id),
            "dashboard",
            "module_settings_updated",
            json!({
                "module_id": module_id,
                "enabled": settings.enabled,
            }),
        )
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
pub async fn moderation_cases(
    State(state): State<WebState>,
    Path((guild_id, target_user_id)): Path<(String, String)>,
    Query(query): Query<ModerationCasesQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<ModerationCase>>, ApiError> {
    let (_, session) = authenticate(&state, &headers)?;
    ensure_guild_access(&session, &guild_id)?;
    validate_discord_id(&target_user_id)?;
    if !(1..=25).contains(&query.limit) {
        return Err(ApiError::bad_request(
            "Limit spraw musi mieścić się w zakresie 1–25.",
        ));
    }

    let cases = state
        .app
        .storage
        .list_moderation_cases(
            &guild_id,
            &target_user_id,
            query.include_resolved,
            query.limit,
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(Json(cases))
}

pub async fn resolve_moderation_case(
    State(state): State<WebState>,
    Path((guild_id, case_id)): Path<(String, i64)>,
    headers: HeaderMap,
    Json(payload): Json<ResolveModerationCaseRequest>,
) -> Result<Json<ModerationCase>, ApiError> {
    let (_, session) = authenticate(&state, &headers)?;
    ensure_guild_access(&session, &guild_id)?;
    verify_csrf(&headers, &session)?;
    if case_id <= 0 {
        return Err(ApiError::bad_request(
            "Identyfikator sprawy musi być większy od zera.",
        ));
    }
    let resolution_length = payload.resolution.chars().count();
    if payload.resolution.trim().is_empty() || resolution_length > 512 {
        return Err(ApiError::bad_request(
            "Rozstrzygnięcie musi zawierać od 1 do 512 znaków.",
        ));
    }

    let moderation_case = state
        .app
        .storage
        .resolve_moderation_case(&guild_id, case_id, &session.user.id, &payload.resolution)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .ok_or_else(|| ApiError::not_found("Sprawa nie istnieje albo została już zamknięta."))?;

    state
        .app
        .storage
        .record_audit(
            Some(&guild_id),
            Some(&session.user.id),
            "dashboard",
            "moderation_case_resolved",
            json!({
                "case_id": moderation_case.id,
                "target_user_id": moderation_case.target_user_id,
            }),
        )
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(moderation_case))
}

fn validate_discord_id(value: &str) -> Result<(), ApiError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ApiError::bad_request("Nieprawidłowy identyfikator Discorda."))?;
    if parsed == 0 {
        return Err(ApiError::bad_request(
            "Identyfikator Discorda nie może być zerem.",
        ));
    }
    Ok(())
}

const fn default_moderation_case_limit() -> u8 {
    10
}

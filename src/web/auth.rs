use axum::{
    Json,
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Redirect, Response},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{WebState, routes::ApiError};

const SESSION_COOKIE: &str = "zuckerbot_session";
const DISCORD_AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const DISCORD_TOKEN_URL: &str = "https://discord.com/api/v10/oauth2/token";
const DISCORD_USER_URL: &str = "https://discord.com/api/v10/users/@me";
const DISCORD_GUILDS_URL: &str = "https://discord.com/api/v10/users/@me/guilds";
const ADMINISTRATOR: u64 = 1 << 3;
const MANAGE_GUILD: u64 = 1 << 5;

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub user: DashboardUser,
    pub guilds: Vec<DashboardGuild>,
    pub csrf_token: String,
    #[serde(skip)]
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner: bool,
    pub permissions: String,
}

#[derive(Deserialize)]
pub struct OAuthCallback {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct OAuthToken {
    access_token: String,
}

pub async fn login(State(state): State<WebState>) -> Result<Redirect, ApiError> {
    let csrf_state = Uuid::new_v4().to_string();
    state.oauth_states.insert(csrf_state.clone(), Utc::now());

    let mut url = Url::parse(DISCORD_AUTHORIZE_URL)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", &state.app.config.discord_client_id)
        .append_pair("redirect_uri", &state.app.config.discord_oauth_redirect_url)
        .append_pair("response_type", "code")
        .append_pair("scope", "identify guilds")
        .append_pair("state", &csrf_state)
        .append_pair("prompt", "none");

    Ok(Redirect::temporary(url.as_str()))
}

pub async fn callback(
    State(state): State<WebState>,
    Query(query): Query<OAuthCallback>,
) -> Result<Response, ApiError> {
    let Some((_, created_at)) = state.oauth_states.remove(&query.state) else {
        return Err(ApiError::bad_request("Nieprawidłowy stan logowania OAuth2."));
    };
    if Utc::now() - created_at > Duration::minutes(10) {
        return Err(ApiError::bad_request("Próba logowania OAuth2 wygasła."));
    }

    let token = state
        .app
        .http_client
        .post(DISCORD_TOKEN_URL)
        .basic_auth(
            &state.app.config.discord_client_id,
            Some(&state.app.config.discord_client_secret),
        )
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", query.code.as_str()),
            (
                "redirect_uri",
                state.app.config.discord_oauth_redirect_url.as_str(),
            ),
        ])
        .send()
        .await
        .map_err(ApiError::upstream)?
        .error_for_status()
        .map_err(ApiError::upstream)?
        .json::<OAuthToken>()
        .await
        .map_err(ApiError::upstream)?;

    let user = state
        .app
        .http_client
        .get(DISCORD_USER_URL)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(ApiError::upstream)?
        .error_for_status()
        .map_err(ApiError::upstream)?
        .json::<DashboardUser>()
        .await
        .map_err(ApiError::upstream)?;

    let guilds = state
        .app
        .http_client
        .get(DISCORD_GUILDS_URL)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(ApiError::upstream)?
        .error_for_status()
        .map_err(ApiError::upstream)?
        .json::<Vec<DashboardGuild>>()
        .await
        .map_err(ApiError::upstream)?
        .into_iter()
        .filter(can_manage)
        .collect::<Vec<_>>();

    let session_id = Uuid::new_v4().to_string();
    let session = Session {
        user,
        guilds,
        csrf_token: Uuid::new_v4().to_string(),
        expires_at: Utc::now() + Duration::seconds(state.app.config.session_ttl_seconds),
    };
    state.sessions.insert(session_id.clone(), session);

    let mut response = Redirect::to("/").into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie(
            &session_id,
            state.app.config.session_ttl_seconds,
            state.app.config.session_cookie_secure,
        ))
        .map_err(|error| ApiError::internal(error.to_string()))?,
    );
    Ok(response)
}

pub async fn current_session(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Json<Session>, ApiError> {
    let (_, session) = authenticate(&state, &headers)?;
    Ok(Json(session))
}

pub async fn logout(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (session_id, session) = authenticate(&state, &headers)?;
    verify_csrf(&headers, &session)?;
    state.sessions.remove(&session_id);

    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&expired_session_cookie(
            state.app.config.session_cookie_secure,
        ))
        .map_err(|error| ApiError::internal(error.to_string()))?,
    );
    Ok(response)
}

pub fn authenticate(
    state: &WebState,
    headers: &HeaderMap,
) -> Result<(String, Session), ApiError> {
    let session_id = cookie_value(headers, SESSION_COOKIE)
        .ok_or_else(|| ApiError::unauthorized("Zaloguj się przez Discord."))?;
    let session = state
        .sessions
        .get(&session_id)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| ApiError::unauthorized("Sesja wygasła. Zaloguj się ponownie."))?;

    if session.expires_at <= Utc::now() {
        state.sessions.remove(&session_id);
        return Err(ApiError::unauthorized(
            "Sesja wygasła. Zaloguj się ponownie.",
        ));
    }

    Ok((session_id, session))
}

pub fn verify_csrf(headers: &HeaderMap, session: &Session) -> Result<(), ApiError> {
    let provided = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok());
    if provided != Some(session.csrf_token.as_str()) {
        return Err(ApiError::forbidden("Nieprawidłowy token CSRF."));
    }
    Ok(())
}

pub fn ensure_guild_access(session: &Session, guild_id: &str) -> Result<(), ApiError> {
    if session.guilds.iter().any(|guild| guild.id == guild_id) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Nie masz uprawnień do zarządzania tym serwerem.",
        ))
    }
}

fn can_manage(guild: &DashboardGuild) -> bool {
    if guild.owner {
        return true;
    }
    guild
        .permissions
        .parse::<u64>()
        .is_ok_and(|bits| bits & (ADMINISTRATOR | MANAGE_GUILD) != 0)
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

fn session_cookie(session_id: &str, max_age: i64, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={session_id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if secure { "; Secure" } else { "" }
    )
}

fn expired_session_cookie(secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    )
}

use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

use axum::{
    Json,
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
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

pub const MAX_PENDING_OAUTH_STATES: usize = 1_000;
pub const MAX_LOGIN_REQUESTS_PER_MINUTE_PER_IP: u32 = 10;
pub const MAX_GLOBAL_LOGIN_REQUESTS_PER_MINUTE: u32 = 60;
pub const GUILD_PERMISSIONS_CACHE_TTL_SECONDS: i64 = 60;

#[derive(Clone, Debug)]
pub struct CachedGuildPermission {
    pub can_manage: bool,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct LoginRateLimiter {
    per_ip: DashMap<String, (DateTime<Utc>, u32)>,
    global: AtomicU32,
    global_window_start: AtomicI64,
}

impl LoginRateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            per_ip: DashMap::new(),
            global: AtomicU32::new(0),
            global_window_start: AtomicI64::new(Utc::now().timestamp()),
        }
    }

    pub fn check_and_record(&self, ip: &str) -> Result<(), ApiError> {
        let now = Utc::now();
        let now_ts = now.timestamp();

        let window_start = self.global_window_start.load(Ordering::Relaxed);
        if now_ts.saturating_sub(window_start) >= 60 {
            self.global_window_start.store(now_ts, Ordering::Relaxed);
            self.global.store(1, Ordering::Relaxed);
        } else {
            let count = self.global.fetch_add(1, Ordering::Relaxed);
            if count >= MAX_GLOBAL_LOGIN_REQUESTS_PER_MINUTE {
                return Err(ApiError::too_many_requests(
                    "Zbyt wiele prób logowania na serwerze. Spróbuj ponownie za chwilę.",
                ));
            }
        }

        let mut entry = self.per_ip.entry(ip.to_owned()).or_insert_with(|| (now, 0));
        let (ip_window_start, count) = entry.value_mut();
        if now - *ip_window_start >= Duration::seconds(60) {
            *ip_window_start = now;
            *count = 1;
        } else {
            *count = count.saturating_add(1);
            if *count > MAX_LOGIN_REQUESTS_PER_MINUTE_PER_IP {
                return Err(ApiError::too_many_requests(
                    "Zbyt wiele prób logowania z Twojego adresu IP. Odczekaj chwilę.",
                ));
            }
        }

        Ok(())
    }

    pub fn cleanup_old_windows(&self) {
        let cutoff = Utc::now() - Duration::seconds(120);
        self.per_ip.retain(|_, (timestamp, _)| *timestamp > cutoff);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Session {
    pub user: DashboardUser,
    pub guilds: Vec<DashboardGuild>,
    pub csrf_token: String,
    #[serde(skip)]
    pub expires_at: chrono::DateTime<Utc>,
    #[serde(skip)]
    pub access_token: Option<String>,
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

pub async fn login(
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Result<Redirect, ApiError> {
    let client_ip = extract_client_ip(&headers);
    state.login_rate_limiter.check_and_record(&client_ip)?;

    if state.oauth_states.len() >= MAX_PENDING_OAUTH_STATES {
        cleanup_stale_records(&state);
        if state.oauth_states.len() >= MAX_PENDING_OAUTH_STATES {
            return Err(ApiError::too_many_requests(
                "Osiągnięto limit oczekujących prób logowania. Spróbuj ponownie za chwilę.",
            ));
        }
    }

    let csrf_state = Uuid::new_v4().to_string();
    state.oauth_states.insert(csrf_state.clone(), Utc::now());

    let mut url =
        Url::parse(DISCORD_AUTHORIZE_URL).map_err(|error| ApiError::internal(error.to_string()))?;
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
        return Err(ApiError::bad_request(
            "Nieprawidłowy stan logowania OAuth2.",
        ));
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
        access_token: Some(token.access_token),
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
    state
        .permissions_cache
        .retain(|(user_id, _), _| user_id != &session.user.id);

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

pub fn authenticate(state: &WebState, headers: &HeaderMap) -> Result<(String, Session), ApiError> {
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

pub async fn ensure_guild_access(
    state: &WebState,
    session: &Session,
    guild_id: &str,
) -> Result<(), ApiError> {
    if !session.guilds.iter().any(|guild| guild.id == guild_id) {
        return Err(ApiError::forbidden(
            "Nie masz uprawnień do zarządzania tym serwerem.",
        ));
    }

    let cache_key = (session.user.id.clone(), guild_id.to_owned());
    let now = Utc::now();

    if let Some(entry) = state.permissions_cache.get(&cache_key)
        && now - entry.checked_at < Duration::seconds(GUILD_PERMISSIONS_CACHE_TTL_SECONDS)
    {
        if entry.can_manage {
            return Ok(());
        } else {
            return Err(ApiError::forbidden(
                "Uprawnienia do zarządzania tym serwerem zostały cofnięte.",
            ));
        }
    }

    let can_manage = revalidate_guild_permission(state, session, guild_id).await?;
    state.permissions_cache.insert(
        cache_key,
        CachedGuildPermission {
            can_manage,
            checked_at: now,
        },
    );

    if can_manage {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Uprawnienia do zarządzania tym serwerem zostały cofnięte.",
        ))
    }
}

pub async fn revalidate_guild_permission(
    state: &WebState,
    session: &Session,
    guild_id: &str,
) -> Result<bool, ApiError> {
    if state.app.config.discord_token.starts_with("dummy")
        || state.app.config.discord_token.starts_with("mock")
    {
        return Ok(session
            .guilds
            .iter()
            .any(|g| g.id == guild_id && can_manage(g)));
    }

    let Some(ref access_token) = session.access_token else {
        return Err(ApiError::unauthorized(
            "Sesja nie zawiera tokenu autoryzacji Discord. Zaloguj się ponownie.",
        ));
    };

    let response = state
        .app
        .http_client
        .get(DISCORD_GUILDS_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(ApiError::upstream)?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(ApiError::unauthorized(
            "Token autoryzacyjny Discord wygasł lub został odwołany. Zaloguj się ponownie.",
        ));
    }

    let guilds = response
        .error_for_status()
        .map_err(ApiError::upstream)?
        .json::<Vec<DashboardGuild>>()
        .await
        .map_err(ApiError::upstream)?;

    let can_manage_guild = guilds.iter().any(|g| g.id == guild_id && can_manage(g));

    Ok(can_manage_guild)
}

pub fn cleanup_stale_records(state: &WebState) -> (usize, usize, usize) {
    cleanup_stale_data(
        &state.oauth_states,
        &state.sessions,
        &state.login_rate_limiter,
        &state.permissions_cache,
    )
}

pub fn cleanup_stale_data(
    oauth_states: &DashMap<String, DateTime<Utc>>,
    sessions: &DashMap<String, Session>,
    limiter: &LoginRateLimiter,
    permissions_cache: &DashMap<(String, String), CachedGuildPermission>,
) -> (usize, usize, usize) {
    let now = Utc::now();
    let max_state_age = Duration::minutes(10);

    let before_states = oauth_states.len();
    oauth_states.retain(|_, created_at| now - *created_at <= max_state_age);
    let states_removed = before_states.saturating_sub(oauth_states.len());

    let before_sessions = sessions.len();
    sessions.retain(|_, session| session.expires_at > now);
    let sessions_removed = before_sessions.saturating_sub(sessions.len());

    limiter.cleanup_old_windows();

    let before_perms = permissions_cache.len();
    let perms_cutoff = Duration::seconds(GUILD_PERMISSIONS_CACHE_TTL_SECONDS * 2);
    permissions_cache.retain(|_, entry| now - entry.checked_at <= perms_cutoff);
    let perms_removed = before_perms.saturating_sub(permissions_cache.len());

    (states_removed, sessions_removed, perms_removed)
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("127.0.0.1")
        .to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_config() -> crate::config::Config {
        crate::config::Config {
            discord_token: "dummy-token".to_owned(),
            discord_application_id: 1,
            discord_client_id: "1".to_owned(),
            discord_client_secret: "secret".to_owned(),
            discord_oauth_redirect_url: "http://localhost/callback".to_owned(),
            discord_dev_guild_id: None,
            dashboard_bind: "127.0.0.1:8080".parse().unwrap(),
            dashboard_public_url: "http://localhost:8080".to_owned(),
            database_url: None,
            scripts_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
            session_cookie_secure: false,
            session_ttl_seconds: 604800,
            lua_memory_limit_bytes: 4 * 1024 * 1024,
            lua_instruction_limit: 200_000,
            lua_hook_granularity: 100,
            music_allowed_hosts: std::collections::HashSet::new(),
        }
    }

    #[test]
    fn test_login_rate_limiter_blocks_excessive_attempts() {
        let limiter = LoginRateLimiter::new();
        let ip = "192.168.1.100";
        for _ in 0..MAX_LOGIN_REQUESTS_PER_MINUTE_PER_IP {
            assert!(limiter.check_and_record(ip).is_ok());
        }
        let blocked = limiter.check_and_record(ip);
        assert!(blocked.is_err());
        assert_eq!(blocked.unwrap_err().status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_cleanup_stale_records() {
        let sessions = DashMap::new();
        let oauth_states = DashMap::new();
        let limiter = LoginRateLimiter::new();

        oauth_states.insert("active_state".to_owned(), Utc::now());
        oauth_states.insert("stale_state".to_owned(), Utc::now() - Duration::minutes(15));

        let user = DashboardUser {
            id: "123".to_owned(),
            username: "tester".to_owned(),
            global_name: None,
            avatar: None,
        };

        sessions.insert(
            "active_session".to_owned(),
            Session {
                user: user.clone(),
                guilds: vec![],
                csrf_token: "csrf1".to_owned(),
                expires_at: Utc::now() + Duration::hours(1),
                access_token: None,
            },
        );

        sessions.insert(
            "expired_session".to_owned(),
            Session {
                user,
                guilds: vec![],
                csrf_token: "csrf2".to_owned(),
                expires_at: Utc::now() - Duration::minutes(1),
                access_token: None,
            },
        );

        let perms_cache = DashMap::new();
        perms_cache.insert(
            ("123".to_owned(), "guild_stale".to_owned()),
            CachedGuildPermission {
                can_manage: true,
                checked_at: Utc::now() - Duration::minutes(10),
            },
        );

        let (st_cleaned, sess_cleaned, perms_cleaned) =
            cleanup_stale_data(&oauth_states, &sessions, &limiter, &perms_cache);
        assert_eq!(st_cleaned, 1);
        assert_eq!(sess_cleaned, 1);
        assert_eq!(perms_cleaned, 1);
        assert!(oauth_states.contains_key("active_state"));
        assert!(!oauth_states.contains_key("stale_state"));
        assert!(sessions.contains_key("active_session"));
        assert!(!sessions.contains_key("expired_session"));
        assert!(perms_cache.is_empty());
    }

    #[tokio::test]
    async fn test_ensure_guild_access_allows_and_revokes() {
        let app = crate::state::AppState::bootstrap(test_config())
            .await
            .expect("bootstrap");
        let permissions_cache = Arc::new(DashMap::new());
        let state = WebState {
            app,
            sessions: Arc::new(DashMap::new()),
            oauth_states: Arc::new(DashMap::new()),
            login_rate_limiter: Arc::new(LoginRateLimiter::new()),
            permissions_cache: permissions_cache.clone(),
        };

        let mut session = Session {
            user: DashboardUser {
                id: "user_42".to_owned(),
                username: "admin".to_owned(),
                global_name: None,
                avatar: None,
            },
            guilds: vec![DashboardGuild {
                id: "guild_1".to_owned(),
                name: "Server 1".to_owned(),
                icon: None,
                owner: false,
                permissions: "32".to_owned(), // MANAGE_GUILD
            }],
            csrf_token: "csrf_token".to_owned(),
            expires_at: Utc::now() + Duration::days(7),
            access_token: Some("mock_oauth_token".to_owned()),
        };

        // 1. Initial check: user has permission -> should succeed and populate cache
        let initial_check = ensure_guild_access(&state, &session, "guild_1").await;
        assert!(initial_check.is_ok());
        assert!(permissions_cache.contains_key(&("user_42".to_owned(), "guild_1".to_owned())));
        let cached = permissions_cache
            .get(&("user_42".to_owned(), "guild_1".to_owned()))
            .unwrap();
        assert!(cached.can_manage);
        drop(cached);

        // 2. Fast cache hit: still permitted
        let cache_hit = ensure_guild_access(&state, &session, "guild_1").await;
        assert!(cache_hit.is_ok());

        // 3. Admin revokes permission in Discord (simulated here by demoting user in guild permissions)
        session.guilds[0].permissions = "0".to_owned();

        // 4. While cache is fresh (< 60s), cache still allows
        let before_expiry = ensure_guild_access(&state, &session, "guild_1").await;
        assert!(before_expiry.is_ok());

        // 5. Expire cache entry (> 60s)
        permissions_cache.insert(
            ("user_42".to_owned(), "guild_1".to_owned()),
            CachedGuildPermission {
                can_manage: true,
                checked_at: Utc::now() - Duration::seconds(65),
            },
        );

        // 6. Next write/sensitive check revalidates and fails closed with 403 Forbidden!
        let after_expiry = ensure_guild_access(&state, &session, "guild_1").await;
        assert!(after_expiry.is_err());
        let err = after_expiry.unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert_eq!(
            err.message,
            "Uprawnienia do zarządzania tym serwerem zostały cofnięte."
        );

        // 7. Verify cache now records can_manage = false
        let cached_revoked = permissions_cache
            .get(&("user_42".to_owned(), "guild_1".to_owned()))
            .unwrap();
        assert!(!cached_revoked.can_manage);
    }
}

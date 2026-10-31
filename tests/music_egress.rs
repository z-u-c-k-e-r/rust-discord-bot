use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use zuckerbot::AppState;
use zuckerbot::config::Config;
use zuckerbot::discord::music::validate_source;

#[derive(Clone, Default)]
struct MockMediaServerState {
    internal_target_hit_count: Arc<AtomicUsize>,
    public_target_hit_count: Arc<AtomicUsize>,
}

async fn handle_redirect_to_metadata() -> impl IntoResponse {
    Redirect::temporary("https://169.254.169.254/latest/meta-data")
}

async fn handle_redirect_to_non_https() -> impl IntoResponse {
    Redirect::temporary("http://example.com/unencrypted")
}

async fn handle_redirect_to_loopback() -> impl IntoResponse {
    Redirect::temporary("https://127.0.0.1:9999/secret")
}

async fn handle_internal_secret(State(state): State<MockMediaServerState>) -> Response {
    state
        .internal_target_hit_count
        .fetch_add(1, Ordering::SeqCst);
    StatusCode::OK.into_response()
}

async fn handle_allowed_media_stream(State(state): State<MockMediaServerState>) -> Response {
    state.public_target_hit_count.fetch_add(1, Ordering::SeqCst);
    (StatusCode::OK, "mock_audio_stream_data").into_response()
}

#[tokio::test]
async fn test_music_resolver_network_egress_and_ssrf_boundary() {
    let mock_state = MockMediaServerState::default();
    let app = Router::new()
        .route("/redirect_metadata", get(handle_redirect_to_metadata))
        .route("/redirect_non_https", get(handle_redirect_to_non_https))
        .route("/redirect_loopback", get(handle_redirect_to_loopback))
        .route("/internal_secret", get(handle_internal_secret))
        .route("/media.mp3", get(handle_allowed_media_stream))
        .with_state(mock_state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let config = Config {
        discord_token: "dummy_token".to_string(),
        discord_application_id: 1,
        discord_client_id: "1".to_string(),
        discord_client_secret: "secret".to_string(),
        discord_oauth_redirect_url: "http://127.0.0.1:8080/auth/discord/callback".to_string(),
        discord_dev_guild_id: None,
        dashboard_bind: "127.0.0.1:0".parse().unwrap(),
        dashboard_public_url: "http://127.0.0.1:8080".to_string(),
        database_url: None,
        scripts_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        session_cookie_secure: false,
        session_ttl_seconds: 86400,
        lua_memory_limit_bytes: 4 * 1024 * 1024,
        lua_instruction_limit: 200_000,
        lua_hook_granularity: 100,
        music_allowed_hosts: ["youtube.com".to_string(), "127.0.0.1".to_string()]
            .into_iter()
            .collect(),
        trusted_proxies: ["127.0.0.1".parse().unwrap()].into_iter().collect(),
    };

    let state = AppState::bootstrap(config).await.expect("bootstrap");

    // 1. Validate source rejects blocked internal IP addresses at application layer
    let blocked_urls = [
        "https://127.0.0.1/audio.mp3",
        "https://10.0.0.1/audio.mp3",
        "https://172.16.0.1/audio.mp3",
        "https://192.168.1.1/audio.mp3",
        "https://169.254.169.254/audio.mp3",
    ];
    for url in blocked_urls {
        assert!(
            validate_source(&state, url).is_err(),
            "validate_source must reject private/loopback IP literal: {url}"
        );
    }

    // 2. Test egress redirect policy: media client refuses redirect to cloud metadata IP
    let res_metadata = state
        .media_http_client
        .get(format!("http://127.0.0.1:{port}/redirect_metadata"))
        .send()
        .await;
    assert!(
        res_metadata.is_err(),
        "redirect to cloud metadata IP 169.254.169.254 must be blocked by redirect policy"
    );

    // 3. Test egress redirect policy: media client refuses redirect to loopback IP literal
    let res_loopback = state
        .media_http_client
        .get(format!("http://127.0.0.1:{port}/redirect_loopback"))
        .send()
        .await;
    assert!(
        res_loopback.is_err(),
        "redirect to loopback 127.0.0.1 must be blocked by redirect policy"
    );

    // 4. Test egress redirect policy: media client refuses redirect to non-HTTPS
    let res_non_https = state
        .media_http_client
        .get(format!("http://127.0.0.1:{port}/redirect_non_https"))
        .send()
        .await;
    assert!(
        res_non_https.is_err(),
        "redirect to unencrypted HTTP must be blocked by redirect policy"
    );

    // Assert that the internal secret target was NEVER hit
    assert_eq!(
        mock_state.internal_target_hit_count.load(Ordering::SeqCst),
        0,
        "internal destination must never be contacted"
    );

    // 5. Permitted media download stream succeeds
    let res_allowed = state
        .media_http_client
        .get(format!("http://127.0.0.1:{port}/media.mp3"))
        .send()
        .await;
    assert!(res_allowed.is_ok(), "allowed media download must succeed");
    let text = res_allowed.unwrap().text().await.unwrap();
    assert_eq!(text, "mock_audio_stream_data");
    assert_eq!(mock_state.public_target_hit_count.load(Ordering::SeqCst), 1);
}

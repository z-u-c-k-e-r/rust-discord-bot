use std::{collections::HashSet, net::SocketAddr, path::PathBuf, time::Duration};

use zuckerbot::{AppState, config::Config};

fn test_config(bind: SocketAddr) -> Config {
    Config {
        discord_token: "dummy_token".to_string(),
        discord_application_id: 123456789,
        discord_client_id: "123456789".to_string(),
        discord_client_secret: "secret".to_string(),
        discord_oauth_redirect_url: "http://127.0.0.1:8080/auth/discord/callback".to_string(),
        discord_dev_guild_id: None,
        dashboard_bind: bind,
        dashboard_public_url: format!("http://{}", bind),
        database_url: None,
        scripts_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        session_cookie_secure: false,
        session_ttl_seconds: 86400,
        lua_memory_limit_bytes: 4 * 1024 * 1024,
        lua_instruction_limit: 200_000,
        lua_hook_granularity: 100,
        music_allowed_hosts: HashSet::new(),
        trusted_proxies: ["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()]
            .into_iter()
            .collect(),
    }
}

#[tokio::test]
async fn test_health_endpoints_metrics_and_graceful_shutdown() {
    // Pick an ephemeral local port
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let config = test_config(addr);
    let state = AppState::bootstrap(config)
        .await
        .expect("bootstrap should succeed");

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let serve_handle = tokio::spawn(zuckerbot::web::serve_with_shutdown(
        state.clone(),
        shutdown_rx,
    ));

    // Wait for server to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // Test /health/live
    let res = client
        .get(format!("{base}/health/live"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let live_body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(live_body["status"], "ok");
    assert!(live_body["uptime_seconds"].as_u64().is_some());

    // Test legacy /healthz
    let res = client.get(format!("{base}/healthz")).send().await.unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);

    // Test /health/ready in offline mode (dummy token)
    let res = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let ready_body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(ready_body["status"], "ready");
    assert_eq!(ready_body["checks"]["storage"], "ok");
    assert_eq!(ready_body["checks"]["lifecycle"], "ok (running)");

    // Test /metrics
    let res = client.get(format!("{base}/metrics")).send().await.unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let metrics_text = res.text().await.unwrap();
    assert!(metrics_text.contains("zuckerbot_uptime_seconds"));
    assert!(metrics_text.contains("zuckerbot_dashboard_requests_total"));

    // Test readiness transitions to 503 during shutdown
    state
        .is_shutting_down
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let res_shutting_down = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res_shutting_down.status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        "readiness must return 503 when process is shutting down"
    );

    // Test graceful shutdown
    let _ = shutdown_tx.send(());
    let shutdown_res = tokio::time::timeout(Duration::from_secs(3), serve_handle).await;
    assert!(
        shutdown_res.is_ok(),
        "server should shut down gracefully within timeout"
    );
    let inner = shutdown_res.unwrap().unwrap();
    assert!(inner.is_ok(), "serve should exit cleanly with Ok(())");
}

#[tokio::test]
async fn test_busy_port_fails_to_bind_with_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let config = test_config(addr);
    let state = AppState::bootstrap(config).await.expect("bootstrap");

    let (_tx, rx) = tokio::sync::broadcast::channel(1);
    let res = zuckerbot::web::serve_with_shutdown(state, rx).await;
    assert!(
        res.is_err(),
        "binding to already occupied port must return Err"
    );
}

#[tokio::test]
async fn test_online_readiness_requires_gateway_ready() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let mut config = test_config(addr);
    // Real/online bot token (not starting with dummy/mock)
    config.discord_token = "Bot.ActualDiscordTokenForProductionDeployment".to_string();

    let state = AppState::bootstrap(config).await.expect("bootstrap");
    let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let _serve = tokio::spawn(zuckerbot::web::serve_with_shutdown(
        state.clone(),
        shutdown_rx,
    ));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    // 1. Before gateway Ready event is received: readiness must return 503
    let res_before = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res_before.status(),
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        "readiness must return 503 before gateway is ready"
    );
    let body_before: serde_json::Value = res_before.json().await.unwrap();
    assert_eq!(body_before["status"], "unready");
    assert!(
        body_before["checks"]["discord_gateway"]
            .as_str()
            .unwrap()
            .contains("waiting for gateway ready event")
    );

    // 2. Gateway Ready event received: readiness transitions to 200
    state
        .is_gateway_ready
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let res_after = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res_after.status(),
        reqwest::StatusCode::OK,
        "readiness must return 200 after gateway is ready"
    );
    let body_after: serde_json::Value = res_after.json().await.unwrap();
    assert_eq!(body_after["status"], "ready");
    assert_eq!(
        body_after["checks"]["discord_gateway"],
        "ok (gateway ready)"
    );
}

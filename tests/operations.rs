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

    // Test /health/ready
    let res = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let ready_body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(ready_body["status"], "ready");
    assert_eq!(ready_body["checks"]["storage"], "ok");

    // Test /metrics
    let res = client.get(format!("{base}/metrics")).send().await.unwrap();
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let metrics_text = res.text().await.unwrap();
    assert!(metrics_text.contains("zuckerbot_uptime_seconds"));
    assert!(metrics_text.contains("zuckerbot_dashboard_requests_total"));

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

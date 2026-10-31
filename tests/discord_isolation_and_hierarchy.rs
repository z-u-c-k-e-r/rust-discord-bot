use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Router,
    extract::{State, WebSocketUpgrade},
    http::{Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{any, get},
};
use serde_json::{Value, json};
use serenity::{
    all::{ChannelId, GatewayIntents, GuildId, Ready, UserId},
    client::{Context, EventHandler},
    http::HttpBuilder,
};
use tokio::sync::{Mutex, oneshot};
use zuckerbot::{
    AppState,
    config::Config,
    discord::executor::{
        ActionOrigin, ChannelPurpose, ensure_member_hierarchy, execute_event_actions,
        resolve_and_validate_channel,
    },
    lua::LuaAction,
};

#[derive(Clone, Default)]
struct MockServerState {
    port: u16,
    modifying_requests_count: Arc<AtomicUsize>,
    recorded_calls: Arc<Mutex<Vec<(Method, String)>>>,
}

async fn handle_ws(ws: WebSocketUpgrade) -> Response {
    println!("MOCK WS UPGRADED!");
    ws.on_upgrade(|mut socket| async move {
        println!("MOCK WS SOCKET CONNECTED!");
        // Send OP 10 Hello
        let hello = json!({
            "op": 10,
            "d": {
                "heartbeat_interval": 45000
            }
        });
        if socket
            .send(axum::extract::ws::Message::Text(hello.to_string().into()))
            .await
            .is_err()
        {
            return;
        }

        // Wait for OP 2 Identify
        while let Some(res) = socket.recv().await {
            println!("MOCK WS RECV: {:?}", res);
            if let Ok(msg) = res {
                let text = match msg {
                    axum::extract::ws::Message::Text(t) => t.to_string(),
                    axum::extract::ws::Message::Binary(b) => {
                        String::from_utf8_lossy(&b).to_string()
                    }
                    _ => continue,
                };
                if let Ok(v) = serde_json::from_str::<Value>(&text)
                    && v.get("op").and_then(|op| op.as_i64()) == Some(2)
                {
                    println!("MOCK WS GOT IDENTIFY: sending READY");
                    // Send OP 0 Ready
                    let ready = json!({
                        "op": 0,
                        "s": 1,
                        "t": "READY",
                        "d": {
                            "v": 10,
                            "user": {
                                "id": "999999999999999999",
                                "username": "MockBot",
                                "discriminator": "0000",
                                "avatar": null,
                                "bot": true
                            },
                            "guilds": [],
                            "session_id": "test_session_id",
                            "resume_gateway_url": "ws://127.0.0.1/ws",
                            "application": {
                                "id": "999999999999999999",
                                "flags": 0
                            }
                        }
                    });
                    let _ = socket
                        .send(axum::extract::ws::Message::Text(ready.to_string().into()))
                        .await;
                    break;
                }
            }
        }

        // Keep socket alive while test runs
        while let Some(Ok(_)) = socket.recv().await {}
    })
}

async fn mock_discord_handler(
    State(mock): State<MockServerState>,
    method: Method,
    uri: axum::http::Uri,
) -> Response {
    let path = uri.path().to_string();
    println!("MOCK HTTP RECV: {} {}", method, path);

    if matches!(
        method,
        Method::POST | Method::DELETE | Method::PATCH | Method::PUT
    ) {
        mock.modifying_requests_count.fetch_add(1, Ordering::SeqCst);
    }
    mock.recorded_calls
        .lock()
        .await
        .push((method.clone(), path.clone()));

    // Gateway URL
    if path == "/api/v10/gateway" || path == "/api/v10/gateway/bot" {
        return Json(json!({
            "url": format!("ws://127.0.0.1:{}/gateway/ws", mock.port),
            "shards": 1,
            "session_start_limit": {
                "total": 1000,
                "remaining": 999,
                "reset_after": 14400000,
                "max_concurrency": 1
            }
        }))
        .into_response();
    }

    // Bot user
    if path == "/api/v10/users/@me" {
        return Json(json!({
            "id": "999999999999999999",
            "username": "MockBot",
            "discriminator": "0000",
            "avatar": null,
            "bot": true
        }))
        .into_response();
    }

    // Channel queries: GET /api/v10/channels/{channel_id}
    if method == Method::GET && path.starts_with("/api/v10/channels/") {
        let channel_id = path.trim_start_matches("/api/v10/channels/");
        return match channel_id {
            "101" => Json(json!({
                "id": "101",
                "guild_id": "100",
                "type": 0, // GuildText
                "name": "general",
                "position": 0
            }))
            .into_response(),
            "201" => Json(json!({
                "id": "201",
                "guild_id": "200", // Foreign guild
                "type": 0,
                "name": "other-guild-text",
                "position": 0
            }))
            .into_response(),
            "301" => Json(json!({
                "id": "301",
                "type": 1, // DM
                "recipients": [{
                    "id": "555555555555555555",
                    "username": "testuser"
                }]
            }))
            .into_response(),
            "401" => Json(json!({
                "id": "401",
                "guild_id": "100",
                "type": 4, // Category
                "name": "MY CATEGORY",
                "position": 0
            }))
            .into_response(),
            _ => (
                StatusCode::NOT_FOUND,
                Json(json!({"message": "Unknown Channel", "code": 10003})),
            )
                .into_response(),
        };
    }

    // Send Message: POST /api/v10/channels/{channel_id}/messages
    if method == Method::POST && path.contains("/messages") {
        return (
            StatusCode::OK,
            Json(json!({
                "id": "900000000000000001",
                "channel_id": "101",
                "author": {
                    "id": "999999999999999999",
                    "username": "MockBot",
                    "avatar": null,
                    "bot": true
                },
                "content": "message",
                "timestamp": "2026-10-31T07:00:00Z",
                "tts": false,
                "mention_everyone": false,
                "mentions": [],
                "mention_roles": [],
                "attachments": [],
                "embeds": [],
                "pinned": false,
                "type": 0
            })),
        )
            .into_response();
    }

    // Guild query: GET /api/v10/guilds/{guild_id}
    if method == Method::GET && (path == "/api/v10/guilds/100" || path == "/api/v10/guilds/100/") {
        return Json(json!({
            "id": "100",
            "name": "Test Guild",
            "owner_id": "111111111111111111", // Server Owner
            "verification_level": 0,
            "default_message_notifications": 0,
            "explicit_content_filter": 0,
            "afk_timeout": 300,
            "mfa_level": 0,
            "system_channel_flags": 0,
            "nsfw_level": 0,
            "premium_tier": 0,
            "preferred_locale": "en-US",
            "emojis": [],
            "stickers": [],
            "features": [],
            "premium_progress_bar_enabled": false,
            "roles": [
                {
                    "id": "1001",
                    "name": "AdminRole",
                    "position": 50,
                    "permissions": "8", // Administrator
                    "color": 0,
                    "colors": {
                        "primary_color": 0
                    },
                    "hoist": false,
                    "managed": false,
                    "mentionable": false,
                    "flags": 0
                },
                {
                    "id": "1002",
                    "name": "MemberRole",
                    "position": 10,
                    "permissions": "0",
                    "color": 0,
                    "colors": {
                        "primary_color": 0
                    },
                    "hoist": false,
                    "managed": false,
                    "mentionable": false,
                    "flags": 0
                }
            ]
        }))
        .into_response();
    }

    // Guild Member queries: GET /api/v10/guilds/100/members/{user_id}
    if method == Method::GET && path.starts_with("/api/v10/guilds/100/members/") {
        let user_id = path.trim_start_matches("/api/v10/guilds/100/members/");
        return match user_id {
            // Owner
            "111111111111111111" => Json(json!({
                "user": { "id": "111111111111111111", "username": "Owner", "avatar": null, "bot": false },
                "roles": ["1001"],
                "joined_at": "2026-01-01T00:00:00Z",
                "deaf": false,
                "mute": false,
                "flags": 0,
                "pending": false
            })).into_response(),
            // Bot
            "999999999999999999" => Json(json!({
                "user": { "id": "999999999999999999", "username": "MockBot", "avatar": null, "bot": true },
                "roles": ["1001"], // Admin role (rank 50)
                "joined_at": "2026-01-01T00:00:00Z",
                "deaf": false,
                "mute": false,
                "flags": 0,
                "pending": false
            })).into_response(),
            // Regular raid target member
            "222222222222222222" => Json(json!({
                "user": { "id": "222222222222222222", "username": "RaidSpammer", "avatar": null, "bot": false },
                "roles": ["1002"], // Member role (rank 10)
                "joined_at": "2026-10-31T06:59:00Z",
                "deaf": false,
                "mute": false,
                "flags": 0,
                "pending": false
            })).into_response(),
            _ => (StatusCode::NOT_FOUND, Json(json!({"message": "Unknown Member", "code": 10007}))).into_response(),
        };
    }

    // Kick: DELETE /api/v10/guilds/100/members/{user_id}
    if method == Method::DELETE && path.starts_with("/api/v10/guilds/100/members/") {
        return StatusCode::NO_CONTENT.into_response();
    }

    // Timeout: PATCH /api/v10/guilds/100/members/{user_id}
    if method == Method::PATCH && path.starts_with("/api/v10/guilds/100/members/") {
        return Json(json!({
            "user": { "id": "222222222222222222", "username": "RaidSpammer" },
            "roles": ["1002"],
            "joined_at": "2026-10-31T06:59:00Z",
            "communication_disabled_until": "2026-10-31T08:00:00Z"
        }))
        .into_response();
    }

    // Default fallback
    (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
}

struct TestReadyHandler {
    tx: Arc<Mutex<Option<oneshot::Sender<Context>>>>,
}

#[serenity::async_trait]
impl EventHandler for TestReadyHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!(
            "Mock Discord Gateway READY received for bot: {}",
            ready.user.name
        );
        if let Some(tx) = self.tx.lock().await.take() {
            let _ = tx.send(ctx);
        }
    }
}

async fn start_mock_discord_environment()
-> (Context, MockServerState, Arc<serenity::all::ShardManager>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let port = listener.local_addr().unwrap().port();
    let mock_state = MockServerState {
        port,
        ..Default::default()
    };

    let app = Router::new()
        .route("/gateway/ws", get(handle_ws))
        .fallback(any(mock_discord_handler))
        .with_state(mock_state.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (ready_tx, ready_rx) = oneshot::channel();
    let handler = TestReadyHandler {
        tx: Arc::new(Mutex::new(Some(ready_tx))),
    };

    let http = HttpBuilder::new("mock-token")
        .proxy(format!("http://127.0.0.1:{port}"))
        .ratelimiter_disabled(true)
        .build();

    let mut client = serenity::client::ClientBuilder::new_with_http(http, GatewayIntents::all())
        .event_handler(handler)
        .await
        .expect("build serenity client");

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        let _ = client.start().await;
    });

    let ctx = tokio::time::timeout(std::time::Duration::from_secs(5), ready_rx)
        .await
        .expect("timed out waiting for gateway ready")
        .expect("channel rx");

    (ctx, mock_state, shard_manager)
}

async fn create_test_state() -> AppState {
    let bind: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = Config {
        discord_token: "dummy_token".to_string(),
        discord_application_id: 123456789,
        discord_client_id: "123456789".to_string(),
        discord_client_secret: "secret".to_string(),
        discord_oauth_redirect_url: "http://127.0.0.1:8080/auth/discord/callback".to_string(),
        discord_dev_guild_id: None,
        dashboard_bind: bind,
        dashboard_public_url: format!("http://{}", bind),
        database_url: None,
        scripts_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        session_cookie_secure: false,
        session_ttl_seconds: 86400,
        lua_memory_limit_bytes: 4 * 1024 * 1024,
        lua_instruction_limit: 200_000,
        lua_hook_granularity: 100,
        music_allowed_hosts: std::collections::HashSet::new(),
        trusted_proxies: ["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()]
            .into_iter()
            .collect(),
    };
    AppState::bootstrap(config)
        .await
        .expect("bootstrap should succeed")
}

#[tokio::test]
async fn test_channel_resolution_server_isolation_and_error_handling() {
    let (ctx, mock, shard_manager) = start_mock_discord_environment().await;
    let origin_guild_id = GuildId::new(100);

    let origin = ActionOrigin {
        guild_id: Some(origin_guild_id),
        channel_id: Some(ChannelId::new(101)),
        actor_id: Some(UserId::new(111111111111111111)),
        actor_permissions: serenity::all::Permissions::all(),
        app_permissions: serenity::all::Permissions::all(),
        enforce_actor_permissions: true,
    };

    // 1. Correct channel on same guild -> Ok
    let resolved =
        resolve_and_validate_channel(&ctx, &origin, Some("101"), ChannelPurpose::SendMessage).await;
    assert!(
        resolved.is_ok(),
        "channel 101 on guild 100 should resolve successfully"
    );
    assert_eq!(resolved.unwrap().get(), 101);

    let baseline_modifications = mock.modifying_requests_count.load(Ordering::SeqCst);

    // 2. Foreign server channel -> Rejected BEFORE sending modifying requests
    let foreign =
        resolve_and_validate_channel(&ctx, &origin, Some("201"), ChannelPurpose::SendMessage).await;
    assert!(foreign.is_err(), "foreign channel 201 should be rejected");
    assert!(foreign.unwrap_err().to_string().contains("obcego serwera"));
    assert_eq!(
        mock.modifying_requests_count.load(Ordering::SeqCst),
        baseline_modifications,
        "Zero modifying requests should be made for foreign channel"
    );

    // 3. DM channel in guild context -> Rejected
    let dm =
        resolve_and_validate_channel(&ctx, &origin, Some("301"), ChannelPurpose::SendMessage).await;
    assert!(
        dm.is_err(),
        "DM channel 301 in guild context should be rejected"
    );
    assert!(dm.unwrap_err().to_string().contains("kanał prywatny"));
    assert_eq!(
        mock.modifying_requests_count.load(Ordering::SeqCst),
        baseline_modifications,
        "Zero modifying requests should be made for DM channel"
    );

    // 4. Category channel -> Rejected for SendMessage
    let category =
        resolve_and_validate_channel(&ctx, &origin, Some("401"), ChannelPurpose::SendMessage).await;
    assert!(
        category.is_err(),
        "Category channel 401 should be rejected for SendMessage"
    );
    assert!(
        category
            .unwrap_err()
            .to_string()
            .contains("kategoria kanałów")
    );
    assert_eq!(
        mock.modifying_requests_count.load(Ordering::SeqCst),
        baseline_modifications,
        "Zero modifying requests should be made for Category channel"
    );

    // 5. Non-existent channel -> Rejected on read error
    let not_found =
        resolve_and_validate_channel(&ctx, &origin, Some("999999"), ChannelPurpose::SendMessage)
            .await;
    assert!(
        not_found.is_err(),
        "Non-existent channel should fail to read"
    );
    assert_eq!(
        mock.modifying_requests_count.load(Ordering::SeqCst),
        baseline_modifications,
        "Zero modifying requests should be made when reading channel fails"
    );

    shard_manager.shutdown_all().await;
}

#[tokio::test]
async fn test_join_guard_raid_kick_timeout_and_self_punishment_contract() {
    let (ctx, mock, shard_manager) = start_mock_discord_environment().await;
    let state = create_test_state().await;
    let guild_id = GuildId::new(100);
    let target_raid_user = UserId::new(222222222222222222);

    let initial_modifications = mock.modifying_requests_count.load(Ordering::SeqCst);

    // Case A: Full event path for kick and timeout from automated Join Guard
    // Actor is the new member (target_raid_user), but automated rules must NOT trigger self-punishment rejection!
    let actions = vec![
        LuaAction::KickMember {
            user_id: target_raid_user.get().to_string(),
            reason: Some("Join Guard: raid lockdown".to_string()),
        },
        LuaAction::SendMessage {
            channel_id: Some("101".to_string()),
            content: "🚨 Staff Alert: user kicked".to_string(),
        },
    ];

    let result = execute_event_actions(
        &ctx,
        &state,
        "join_guard",
        Some(guild_id),
        Some(ChannelId::new(101)),
        Some(target_raid_user),
        &actions,
    )
    .await;

    assert!(
        result.is_ok(),
        "automated event actions must succeed without self-punishment error"
    );

    // Both the Kick (DELETE) and the Alert (POST) must have been registered
    let modifications_after_raid = mock.modifying_requests_count.load(Ordering::SeqCst);
    assert_eq!(
        modifications_after_raid - initial_modifications,
        2,
        "Kick and staff alert message must both be dispatched"
    );

    // Case B: Protected target (Server Owner 111111111111111111)
    // Server owner cannot be moderated, action is denied, but audit entry is created and alert message IS STILL SENT!
    let owner_user = UserId::new(111111111111111111);
    let protected_actions = vec![
        LuaAction::TimeoutMember {
            user_id: owner_user.get().to_string(),
            seconds: 3600,
            reason: Some("Attempt to moderate owner".to_string()),
        },
        LuaAction::SendMessage {
            channel_id: Some("101".to_string()),
            content: "🛡️ Staff Alert: attempted action on owner".to_string(),
        },
    ];

    let baseline = mock.modifying_requests_count.load(Ordering::SeqCst);

    let protected_result = execute_event_actions(
        &ctx,
        &state,
        "join_guard",
        Some(guild_id),
        Some(ChannelId::new(101)),
        Some(owner_user),
        &protected_actions,
    )
    .await;

    assert!(
        protected_result.is_ok(),
        "execute_event_actions gracefully absorbs denied actions"
    );

    // Zero modifying requests for the timeout (owner protected!), exactly 1 for the staff alert!
    let after_protected = mock.modifying_requests_count.load(Ordering::SeqCst);
    assert_eq!(
        after_protected - baseline,
        1,
        "Owner timeout was rejected, but staff alert was still successfully sent"
    );

    // Case C: Manual moderator command - self punishment is strictly rejected
    let self_check = ensure_member_hierarchy(
        &ctx,
        guild_id,
        Some(target_raid_user), // moderator acting manually
        target_raid_user,       // target is self
    )
    .await;
    assert!(
        self_check.is_err(),
        "manual command on self must be rejected"
    );
    assert!(
        self_check
            .unwrap_err()
            .to_string()
            .contains("nie możesz wykonać tej akcji na sobie")
    );

    shard_manager.shutdown_all().await;
}

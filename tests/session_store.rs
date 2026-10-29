use chrono::{Duration, Utc};
use zuckerbot::session::{
    DashboardGuild, DashboardUser, MemorySessionStore, SessionStore, hash_session_id,
};

#[tokio::test]
async fn test_session_store_lifecycle_and_hashing() {
    let store = SessionStore::memory();
    let session_id = "test-session-secret-token-12345";
    let hash = hash_session_id(session_id);
    assert_ne!(session_id, hash);
    assert_eq!(hash.len(), 64); // SHA-256 hex string

    let user = DashboardUser {
        id: "10001".to_string(),
        username: "admin_tester".to_string(),
        global_name: Some("Admin".to_string()),
        avatar: None,
    };
    let guilds = vec![DashboardGuild {
        id: "20001".to_string(),
        name: "Test Guild".to_string(),
        icon: None,
        owner: true,
        permissions: "8".to_string(),
    }];

    // 1. Save session with access_token (which must be discarded)
    let expires_at = Utc::now() + Duration::hours(2);
    store
        .save_session(
            session_id,
            user.clone(),
            guilds.clone(),
            "csrf_secret".to_string(),
            expires_at,
            Some("discord_secret_bearer_token".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(store.count_active_sessions().await.unwrap(), 1);

    // 2. Fetch session and verify details + discarded access token
    let session = store
        .get_session(session_id)
        .await
        .unwrap()
        .expect("session should exist");
    assert_eq!(session.user.id, "10001");
    assert_eq!(session.user.username, "admin_tester");
    assert_eq!(session.guilds.len(), 1);
    assert_eq!(session.guilds[0].id, "20001");
    assert_eq!(session.csrf_token, "csrf_secret");
    assert!(
        session.access_token.is_none(),
        "Discord access token must be discarded at rest"
    );

    // 3. Querying with invalid ID returns None
    assert!(
        store
            .get_session("invalid-session-token")
            .await
            .unwrap()
            .is_none()
    );

    // 4. Test OAuth state single-use consumption
    let oauth_state = "oauth-csrf-state-abc";
    store.save_oauth_state(oauth_state).await.unwrap();
    assert_eq!(store.count_pending_oauth_states().await.unwrap(), 1);

    // First consume succeeds
    assert!(store.consume_oauth_state(oauth_state).await.unwrap());
    // Second consume fails (already consumed)
    assert!(!store.consume_oauth_state(oauth_state).await.unwrap());

    // 5. Delete session (logout)
    store.delete_session(session_id).await.unwrap();
    assert!(store.get_session(session_id).await.unwrap().is_none());
    assert_eq!(store.count_active_sessions().await.unwrap(), 0);
}

#[tokio::test]
async fn test_expired_session_cleanup() {
    let store = MemorySessionStore::new();
    let session_id = "expiring-session";
    let user = DashboardUser {
        id: "999".to_string(),
        username: "expire_user".to_string(),
        global_name: None,
        avatar: None,
    };

    // Save session that expired 10 seconds ago
    store
        .save_session(
            session_id,
            user,
            vec![],
            "csrf".to_string(),
            Utc::now() - Duration::seconds(10),
            None,
        )
        .unwrap();

    // get_session automatically detects expired session and returns None
    assert!(store.get_session(session_id).unwrap().is_none());

    // cleanup_expired_sessions returns cleaned count
    let cleaned = store.cleanup_expired_sessions().unwrap();
    assert_eq!(cleaned, 0); // already removed on get
}

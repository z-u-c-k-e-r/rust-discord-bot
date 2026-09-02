use std::{env, time::Duration};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zuckerbot_persistence::{ControlPlaneStore, PutGuildModule, StoreError};

fn test_store() -> Option<ControlPlaneStore> {
    let database_url = env::var("TEST_DATABASE_URL").ok()?;
    let redis_url = env::var("TEST_REDIS_URL").ok()?;
    Some(
        ControlPlaneStore::new(
            &database_url,
            &redis_url,
            4,
            Duration::from_secs(3),
            Duration::from_secs(30),
        )
        .unwrap(),
    )
}

fn snowflake_from_uuid() -> String {
    let value = Uuid::new_v4().as_u128() % 9_000_000_000_000_000_000_u128;
    format!("{}", value + 1_000_000_000_000_000_000_u128)
}

#[tokio::test]
async fn module_writes_are_versioned_transactional_and_audited() {
    let Some(store) = test_store() else {
        return;
    };
    store.migrate().await.unwrap();

    let guild_id = snowflake_from_uuid();
    let actor_user_id = snowflake_from_uuid();
    let request_id = format!("integration-{}", Uuid::new_v4());
    let created = store
        .put_guild_module(PutGuildModule {
            guild_id: guild_id.clone(),
            module_id: "moderation".to_owned(),
            enabled: true,
            configuration: serde_json::json!({ "mode": "observe" }),
            expected_version: None,
            actor_user_id: actor_user_id.clone(),
            request_id: request_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(created.version, 1);

    let updated = store
        .put_guild_module(PutGuildModule {
            guild_id: guild_id.clone(),
            module_id: "moderation".to_owned(),
            enabled: true,
            configuration: serde_json::json!({ "mode": "enforce" }),
            expected_version: Some(created.version),
            actor_user_id: actor_user_id.clone(),
            request_id: format!("integration-{}", Uuid::new_v4()),
        })
        .await
        .unwrap();
    assert_eq!(updated.version, 2);

    let stale_result = store
        .put_guild_module(PutGuildModule {
            guild_id: guild_id.clone(),
            module_id: "moderation".to_owned(),
            enabled: false,
            configuration: serde_json::json!({}),
            expected_version: Some(1),
            actor_user_id,
            request_id: format!("integration-{}", Uuid::new_v4()),
        })
        .await;
    assert!(matches!(
        stale_result,
        Err(StoreError::VersionConflict {
            current_version: Some(2)
        })
    ));

    let persisted = store
        .get_guild_module(&guild_id, "moderation")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.configuration["mode"], "enforce");
    assert_eq!(
        store
            .audit_event_count(&guild_id, &request_id)
            .await
            .unwrap(),
        1
    );

    store.delete_guild(&guild_id).await.unwrap();
}

#[tokio::test]
async fn concurrent_initial_write_allows_exactly_one_creator() {
    let Some(store) = test_store() else {
        return;
    };
    store.migrate().await.unwrap();

    let guild_id = snowflake_from_uuid();
    let actor_user_id = snowflake_from_uuid();
    let left_store = store.clone();
    let right_store = store.clone();
    let left_guild_id = guild_id.clone();
    let right_guild_id = guild_id.clone();
    let left_actor_user_id = actor_user_id.clone();

    let left = async move {
        left_store
            .put_guild_module(PutGuildModule {
                guild_id: left_guild_id,
                module_id: "roles".to_owned(),
                enabled: true,
                configuration: serde_json::json!({ "writer": "left" }),
                expected_version: None,
                actor_user_id: left_actor_user_id,
                request_id: format!("integration-{}", Uuid::new_v4()),
            })
            .await
    };
    let right = async move {
        right_store
            .put_guild_module(PutGuildModule {
                guild_id: right_guild_id,
                module_id: "roles".to_owned(),
                enabled: true,
                configuration: serde_json::json!({ "writer": "right" }),
                expected_version: None,
                actor_user_id,
                request_id: format!("integration-{}", Uuid::new_v4()),
            })
            .await
    };

    let (left_result, right_result) = tokio::join!(left, right);
    let results = [&left_result, &right_result];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(StoreError::VersionConflict {
                        current_version: Some(1)
                    })
                )
            })
            .count(),
        1
    );

    let persisted = store
        .get_guild_module(&guild_id, "roles")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted.version, 1);
    assert!(matches!(
        persisted.configuration["writer"].as_str(),
        Some("left" | "right")
    ));

    store.delete_guild(&guild_id).await.unwrap();
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct EphemeralFixture {
    value: String,
}

#[tokio::test]
async fn redis_take_is_single_use() {
    let Some(store) = test_store() else {
        return;
    };
    let key = format!("test:oauth:{}", Uuid::new_v4().simple());
    let fixture = EphemeralFixture {
        value: "one-time".to_owned(),
    };

    store
        .put_ephemeral_json(&key, &fixture, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(
        store
            .take_ephemeral_json::<EphemeralFixture>(&key)
            .await
            .unwrap(),
        Some(fixture)
    );
    assert_eq!(
        store
            .take_ephemeral_json::<EphemeralFixture>(&key)
            .await
            .unwrap(),
        None
    );
}

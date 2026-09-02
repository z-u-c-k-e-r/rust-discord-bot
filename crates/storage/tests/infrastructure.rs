use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use zuckerbot_storage::{
    GuildRepository, Infrastructure, InfrastructureOptions, ModuleConfigurationRepository,
    ReadinessState, StorageError, UpdateModuleConfiguration, UpsertGuild,
};

fn infrastructure() -> Infrastructure {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");
    Infrastructure::new(InfrastructureOptions {
        database_url: &database_url,
        redis_url: &redis_url,
        database_max_connections: 5,
        database_acquire_timeout: Duration::from_secs(3),
        dependency_timeout: Duration::from_secs(2),
    })
    .expect("infrastructure configuration should be valid")
}

fn unique_guild_id() -> String {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_millis();
    format!("9{value}")
}

#[tokio::test]
#[ignore = "requires PostgreSQL and Redis"]
async fn migrations_readiness_and_optimistic_updates_work_together() {
    let infrastructure = infrastructure();
    infrastructure.migrate().await.expect("migrations must run");

    let readiness = infrastructure.readiness().await;
    assert_eq!(readiness.status, ReadinessState::Ready);

    let guild_id = unique_guild_id();
    let guild = infrastructure
        .upsert_guild(UpsertGuild {
            guild_id: &guild_id,
            name: Some("Storage integration test"),
            locale: "pl",
        })
        .await
        .expect("guild should be inserted");
    assert_eq!(guild.version, 1);

    let initial_value = json!({"log_channel_id": "123456789012345678"});
    let initial = infrastructure
        .update_module_configuration(UpdateModuleConfiguration {
            guild_id: &guild_id,
            module_id: "moderation",
            enabled: true,
            configuration: &initial_value,
            expected_version: 0,
            updated_by: "integration-test",
        })
        .await
        .expect("first configuration should be inserted");
    assert_eq!(initial.version, 1);

    let changed_value = json!({"log_channel_id": "123456789012345678", "dm_user": true});
    let changed = infrastructure
        .update_module_configuration(UpdateModuleConfiguration {
            guild_id: &guild_id,
            module_id: "moderation",
            enabled: true,
            configuration: &changed_value,
            expected_version: initial.version,
            updated_by: "integration-test",
        })
        .await
        .expect("matching version should update");
    assert_eq!(changed.version, 2);

    let stale_value = json!({});
    let stale = infrastructure
        .update_module_configuration(UpdateModuleConfiguration {
            guild_id: &guild_id,
            module_id: "moderation",
            enabled: false,
            configuration: &stale_value,
            expected_version: initial.version,
            updated_by: "integration-test",
        })
        .await
        .expect_err("stale version must be rejected");
    assert!(matches!(
        stale,
        StorageError::VersionConflict {
            current_version: Some(2)
        }
    ));

    let loaded = infrastructure
        .get_module_configuration(&guild_id, "moderation")
        .await
        .expect("configuration query should succeed")
        .expect("configuration should exist");
    assert_eq!(loaded, changed);
}

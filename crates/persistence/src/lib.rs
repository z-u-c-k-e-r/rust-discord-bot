use std::{fmt, str::FromStr, time::Duration};

use redis::aio::MultiplexedConnection;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions, PgRow},
};
use thiserror::Error;
use tokio::time::timeout;

pub const MAX_MODULE_CONFIGURATION_BYTES: usize = 64 * 1024;
const MAX_EPHEMERAL_KEY_LENGTH: usize = 256;

#[derive(Clone)]
pub struct ControlPlaneStore {
    postgres: PgPool,
    redis: redis::Client,
    dependency_timeout: Duration,
    migration_timeout: Duration,
}

impl fmt::Debug for ControlPlaneStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlPlaneStore")
            .field("postgres", &"PgPool")
            .field("redis", &"RedisClient")
            .field("dependency_timeout", &self.dependency_timeout)
            .field("migration_timeout", &self.migration_timeout)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    Ready,
    Unavailable,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReadinessReport {
    pub ready: bool,
    pub postgres: DependencyState,
    pub redis: DependencyState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuildModuleConfiguration {
    pub guild_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub configuration: Value,
    pub version: i64,
    pub updated_by: Option<String>,
    pub updated_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PutGuildModule {
    pub guild_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub configuration: Value,
    pub expected_version: Option<i64>,
    pub actor_user_id: String,
    pub request_id: String,
}

#[derive(Debug, Error)]
pub enum ConfigurationValidationError {
    #[error("guild_id must be a Discord snowflake represented as a string")]
    InvalidGuildId,
    #[error("actor_user_id must be a Discord snowflake represented as a string")]
    InvalidActorUserId,
    #[error("module_id must contain 1-64 lowercase ASCII letters, digits, '-' or '_'")]
    InvalidModuleId,
    #[error("module configuration must be a JSON object")]
    ConfigurationMustBeObject,
    #[error("module configuration exceeds the {MAX_MODULE_CONFIGURATION_BYTES}-byte limit")]
    ConfigurationTooLarge,
    #[error("expected_version must be greater than zero when supplied")]
    InvalidExpectedVersion,
    #[error("request_id must contain between 1 and 128 characters")]
    InvalidRequestId,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("PostgreSQL operation failed")]
    Postgres(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Redis operation failed")]
    Redis(#[from] redis::RedisError),
    #[error("JSON serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("dependency operation timed out: {0}")]
    Timeout(&'static str),
    #[error("ephemeral key is invalid")]
    InvalidEphemeralKey,
    #[error("ephemeral TTL must be between 1 second and i64::MAX seconds")]
    InvalidTtl,
    #[error(transparent)]
    Validation(#[from] ConfigurationValidationError),
    #[error("module version conflict")]
    VersionConflict { current_version: Option<i64> },
}

impl ControlPlaneStore {
    pub fn new(
        database_url: &str,
        redis_url: &str,
        postgres_max_connections: u32,
        dependency_timeout: Duration,
        migration_timeout: Duration,
    ) -> Result<Self, StoreError> {
        let postgres_options = PgConnectOptions::from_str(database_url)?;
        let postgres = PgPoolOptions::new()
            .max_connections(postgres_max_connections.max(1))
            .acquire_timeout(dependency_timeout)
            .connect_lazy_with(postgres_options);
        let redis = redis::Client::open(redis_url)?;

        Ok(Self {
            postgres,
            redis,
            dependency_timeout,
            migration_timeout,
        })
    }

    pub async fn migrate(&self) -> Result<(), StoreError> {
        timeout(
            self.migration_timeout,
            sqlx::migrate!("../../migrations").run(&self.postgres),
        )
        .await
        .map_err(|_| StoreError::Timeout("database migration"))??;
        Ok(())
    }

    pub async fn readiness(&self) -> ReadinessReport {
        let (postgres, redis) = tokio::join!(self.postgres_state(), self.redis_state());
        ReadinessReport {
            ready: postgres == DependencyState::Ready && redis == DependencyState::Ready,
            postgres,
            redis,
        }
    }

    pub async fn put_ephemeral_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        validate_ephemeral_key(key)?;
        let ttl_seconds = validate_ttl(ttl)?;
        let serialized = serde_json::to_string(value)?;
        let mut connection = self.redis_connection().await?;

        timeout(
            self.dependency_timeout,
            redis::cmd("SET")
                .arg(key)
                .arg(serialized)
                .arg("EX")
                .arg(ttl_seconds)
                .query_async::<()>(&mut connection),
        )
        .await
        .map_err(|_| StoreError::Timeout("Redis SET"))??;
        Ok(())
    }

    pub async fn get_ephemeral_json<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StoreError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.redis_connection().await?;
        let serialized = timeout(
            self.dependency_timeout,
            redis::cmd("GET")
                .arg(key)
                .query_async::<Option<String>>(&mut connection),
        )
        .await
        .map_err(|_| StoreError::Timeout("Redis GET"))??;

        serialized
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(StoreError::from)
    }

    pub async fn take_ephemeral_json<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, StoreError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.redis_connection().await?;
        let serialized = timeout(
            self.dependency_timeout,
            redis::cmd("GETDEL")
                .arg(key)
                .query_async::<Option<String>>(&mut connection),
        )
        .await
        .map_err(|_| StoreError::Timeout("Redis GETDEL"))??;

        serialized
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(StoreError::from)
    }

    pub async fn delete_ephemeral(&self, key: &str) -> Result<bool, StoreError> {
        validate_ephemeral_key(key)?;
        let mut connection = self.redis_connection().await?;
        let deleted = timeout(
            self.dependency_timeout,
            redis::cmd("DEL")
                .arg(key)
                .query_async::<u64>(&mut connection),
        )
        .await
        .map_err(|_| StoreError::Timeout("Redis DEL"))??;
        Ok(deleted > 0)
    }

    pub async fn get_guild_module(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> Result<Option<GuildModuleConfiguration>, StoreError> {
        validate_guild_id(guild_id)?;
        validate_module_id(module_id)?;

        let row = sqlx::query(
            r#"
            SELECT
                guild_id,
                module_id,
                enabled,
                configuration,
                version,
                updated_by,
                EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at_unix
            FROM guild_modules
            WHERE guild_id = $1 AND module_id = $2 AND version > 0
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .fetch_optional(&self.postgres)
        .await?;

        row.as_ref()
            .map(module_from_row)
            .transpose()
            .map_err(StoreError::from)
    }

    pub async fn list_guild_modules(
        &self,
        guild_id: &str,
    ) -> Result<Vec<GuildModuleConfiguration>, StoreError> {
        validate_guild_id(guild_id)?;

        let rows = sqlx::query(
            r#"
            SELECT
                guild_id,
                module_id,
                enabled,
                configuration,
                version,
                updated_by,
                EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at_unix
            FROM guild_modules
            WHERE guild_id = $1 AND version > 0
            ORDER BY module_id
            "#,
        )
        .bind(guild_id)
        .fetch_all(&self.postgres)
        .await?;

        rows.iter()
            .map(module_from_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub async fn put_guild_module(
        &self,
        input: PutGuildModule,
    ) -> Result<GuildModuleConfiguration, StoreError> {
        validate_put_input(&input)?;

        let mut transaction = self.postgres.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO guilds (guild_id)
            VALUES ($1)
            ON CONFLICT (guild_id) DO NOTHING
            "#,
        )
        .bind(&input.guild_id)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO guild_modules (
                guild_id,
                module_id,
                enabled,
                configuration,
                version,
                updated_by,
                updated_at
            )
            VALUES ($1, $2, FALSE, '{}'::jsonb, 0, NULL, NOW())
            ON CONFLICT (guild_id, module_id) DO NOTHING
            "#,
        )
        .bind(&input.guild_id)
        .bind(&input.module_id)
        .execute(&mut *transaction)
        .await?;

        let current_row = sqlx::query(
            r#"
            SELECT
                guild_id,
                module_id,
                enabled,
                configuration,
                version,
                updated_by,
                EXTRACT(EPOCH FROM updated_at)::BIGINT AS updated_at_unix
            FROM guild_modules
            WHERE guild_id = $1 AND module_id = $2
            FOR UPDATE
            "#,
        )
        .bind(&input.guild_id)
        .bind(&input.module_id)
        .fetch_one(&mut *transaction)
        .await?;
        let current = module_from_row(&current_row)?;

        match (current.version, input.expected_version) {
            (0, None) => {}
            (0, Some(_)) => {
                return Err(StoreError::VersionConflict {
                    current_version: None,
                });
            }
            (current_version, Some(expected)) if current_version == expected => {}
            (current_version, _) => {
                return Err(StoreError::VersionConflict {
                    current_version: Some(current_version),
                });
            }
        }

        let next_version = current.version.saturating_add(1);
        let updated_at_unix: i64 = sqlx::query_scalar(
            r#"
            UPDATE guild_modules
            SET
                enabled = $3,
                configuration = $4,
                version = $5,
                updated_by = $6,
                updated_at = NOW()
            WHERE guild_id = $1 AND module_id = $2
            RETURNING EXTRACT(EPOCH FROM updated_at)::BIGINT
            "#,
        )
        .bind(&input.guild_id)
        .bind(&input.module_id)
        .bind(input.enabled)
        .bind(&input.configuration)
        .bind(next_version)
        .bind(&input.actor_user_id)
        .fetch_one(&mut *transaction)
        .await?;

        let updated = GuildModuleConfiguration {
            guild_id: input.guild_id.clone(),
            module_id: input.module_id.clone(),
            enabled: input.enabled,
            configuration: input.configuration,
            version: next_version,
            updated_by: Some(input.actor_user_id.clone()),
            updated_at_unix,
        };
        let before_state = if current.version == 0 {
            None
        } else {
            Some(serde_json::to_value(&current)?)
        };
        let after_state = serde_json::to_value(&updated)?;

        sqlx::query(
            r#"
            INSERT INTO audit_events (
                guild_id,
                actor_user_id,
                source,
                action,
                resource_type,
                resource_id,
                before_state,
                after_state,
                request_id,
                outcome,
                metadata
            )
            VALUES ($1, $2, 'control-api', 'guild_module.upsert', 'guild_module', $3, $4, $5, $6, 'success', $7)
            "#,
        )
        .bind(&updated.guild_id)
        .bind(&input.actor_user_id)
        .bind(&updated.module_id)
        .bind(before_state)
        .bind(after_state)
        .bind(&input.request_id)
        .bind(serde_json::json!({ "version": updated.version }))
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(updated)
    }

    pub async fn audit_event_count(
        &self,
        guild_id: &str,
        request_id: &str,
    ) -> Result<i64, StoreError> {
        validate_guild_id(guild_id)?;
        let count = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM audit_events
            WHERE guild_id = $1 AND request_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(request_id)
        .fetch_one(&self.postgres)
        .await?;
        Ok(count)
    }

    pub async fn delete_guild(&self, guild_id: &str) -> Result<(), StoreError> {
        validate_guild_id(guild_id)?;
        sqlx::query("DELETE FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .execute(&self.postgres)
            .await?;
        Ok(())
    }

    async fn postgres_state(&self) -> DependencyState {
        match timeout(
            self.dependency_timeout,
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&self.postgres),
        )
        .await
        {
            Ok(Ok(1)) => DependencyState::Ready,
            Ok(Ok(_)) | Ok(Err(_)) => DependencyState::Unavailable,
            Err(_) => DependencyState::TimedOut,
        }
    }

    async fn redis_state(&self) -> DependencyState {
        let operation = async {
            let mut connection = self.redis.get_multiplexed_async_connection().await?;
            redis::cmd("PING")
                .query_async::<String>(&mut connection)
                .await
        };

        match timeout(self.dependency_timeout, operation).await {
            Ok(Ok(response)) if response == "PONG" => DependencyState::Ready,
            Ok(Ok(_)) | Ok(Err(_)) => DependencyState::Unavailable,
            Err(_) => DependencyState::TimedOut,
        }
    }

    async fn redis_connection(&self) -> Result<MultiplexedConnection, StoreError> {
        timeout(
            self.dependency_timeout,
            self.redis.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| StoreError::Timeout("Redis connection"))?
        .map_err(StoreError::from)
    }
}

fn module_from_row(row: &PgRow) -> Result<GuildModuleConfiguration, sqlx::Error> {
    Ok(GuildModuleConfiguration {
        guild_id: row.try_get("guild_id")?,
        module_id: row.try_get("module_id")?,
        enabled: row.try_get("enabled")?,
        configuration: row.try_get("configuration")?,
        version: row.try_get("version")?,
        updated_by: row.try_get("updated_by")?,
        updated_at_unix: row.try_get("updated_at_unix")?,
    })
}

fn validate_put_input(input: &PutGuildModule) -> Result<(), ConfigurationValidationError> {
    validate_guild_id(&input.guild_id)?;
    if !is_snowflake(&input.actor_user_id) {
        return Err(ConfigurationValidationError::InvalidActorUserId);
    }
    validate_module_id(&input.module_id)?;
    if !input.configuration.is_object() {
        return Err(ConfigurationValidationError::ConfigurationMustBeObject);
    }
    if serde_json::to_vec(&input.configuration).map_or(true, |serialized| {
        serialized.len() > MAX_MODULE_CONFIGURATION_BYTES
    }) {
        return Err(ConfigurationValidationError::ConfigurationTooLarge);
    }
    if input.expected_version.is_some_and(|version| version <= 0) {
        return Err(ConfigurationValidationError::InvalidExpectedVersion);
    }
    if !(1..=128).contains(&input.request_id.len()) {
        return Err(ConfigurationValidationError::InvalidRequestId);
    }
    Ok(())
}

fn validate_guild_id(value: &str) -> Result<(), ConfigurationValidationError> {
    if is_snowflake(value) {
        Ok(())
    } else {
        Err(ConfigurationValidationError::InvalidGuildId)
    }
}

fn validate_module_id(value: &str) -> Result<(), ConfigurationValidationError> {
    let is_valid = (1..=64).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        });
    if is_valid {
        Ok(())
    } else {
        Err(ConfigurationValidationError::InvalidModuleId)
    }
}

fn is_snowflake(value: &str) -> bool {
    (1..=20).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn validate_ephemeral_key(key: &str) -> Result<(), StoreError> {
    let is_valid = (1..=MAX_EPHEMERAL_KEY_LENGTH).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'));
    if is_valid {
        Ok(())
    } else {
        Err(StoreError::InvalidEphemeralKey)
    }
}

fn validate_ttl(ttl: Duration) -> Result<u64, StoreError> {
    let seconds = ttl.as_secs();
    if seconds == 0 || seconds > i64::MAX as u64 {
        Err(StoreError::InvalidTtl)
    } else {
        Ok(seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_input() -> PutGuildModule {
        PutGuildModule {
            guild_id: "123456789012345678".to_owned(),
            module_id: "moderation-core".to_owned(),
            enabled: true,
            configuration: serde_json::json!({ "log_channel_id": "987654321098765432" }),
            expected_version: None,
            actor_user_id: "111111111111111111".to_owned(),
            request_id: "request-1".to_owned(),
        }
    }

    #[test]
    fn accepts_a_valid_module_write_contract() {
        assert!(validate_put_input(&valid_input()).is_ok());
    }

    #[test]
    fn rejects_non_object_configuration() {
        let mut input = valid_input();
        input.configuration = serde_json::json!(["not", "an", "object"]);
        assert!(matches!(
            validate_put_input(&input),
            Err(ConfigurationValidationError::ConfigurationMustBeObject)
        ));
    }

    #[test]
    fn rejects_unsafe_module_identifiers() {
        let mut input = valid_input();
        input.module_id = "../secrets".to_owned();
        assert!(matches!(
            validate_put_input(&input),
            Err(ConfigurationValidationError::InvalidModuleId)
        ));
    }

    #[test]
    fn validates_ephemeral_keys_and_ttls() {
        assert!(validate_ephemeral_key("oauth:state:abc_DEF-123").is_ok());
        assert!(validate_ephemeral_key("oauth state").is_err());
        assert_eq!(validate_ttl(Duration::from_secs(60)).unwrap(), 60);
        assert!(validate_ttl(Duration::ZERO).is_err());
    }
}

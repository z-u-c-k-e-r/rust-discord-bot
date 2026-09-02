use std::{
    fmt,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool, postgres::PgPoolOptions};
use thiserror::Error;
use tokio::time::timeout;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
const MAX_CONFIGURATION_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct Infrastructure {
    postgres: PgPool,
    redis: redis::Client,
    dependency_timeout: Duration,
}

#[derive(Clone)]
pub struct InfrastructureOptions<'a> {
    pub database_url: &'a str,
    pub redis_url: &'a str,
    pub database_max_connections: u32,
    pub database_acquire_timeout: Duration,
    pub dependency_timeout: Duration,
}

impl fmt::Debug for InfrastructureOptions<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InfrastructureOptions")
            .field("database_url", &"[REDACTED]")
            .field("redis_url", &"[REDACTED]")
            .field("database_max_connections", &self.database_max_connections)
            .field("database_acquire_timeout", &self.database_acquire_timeout)
            .field("dependency_timeout", &self.dependency_timeout)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    NotReady,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    Ok,
    Unavailable,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DependencyCheck {
    pub state: DependencyState,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReadinessReport {
    pub status: ReadinessState,
    pub postgres: DependencyCheck,
    pub redis: DependencyCheck,
}

#[derive(Clone, Debug, FromRow, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildRecord {
    pub guild_id: String,
    pub name: Option<String>,
    pub locale: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpsertGuild<'a> {
    pub guild_id: &'a str,
    pub name: Option<&'a str>,
    pub locale: &'a str,
}

#[derive(Clone, Debug, FromRow, PartialEq, Serialize, Deserialize)]
pub struct ModuleConfiguration {
    pub guild_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub configuration: Value,
    pub version: i64,
    pub updated_by: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateModuleConfiguration<'a> {
    pub guild_id: &'a str,
    pub module_id: &'a str,
    pub enabled: bool,
    pub configuration: &'a Value,
    pub expected_version: i64,
    pub updated_by: &'a str,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Redis configuration is invalid")]
    RedisConfiguration(#[from] redis::RedisError),
    #[error("invalid storage input: {0}")]
    Validation(&'static str),
    #[error("configuration version conflict; current version: {current_version:?}")]
    VersionConflict { current_version: Option<i64> },
}

#[async_trait]
pub trait GuildRepository: Send + Sync {
    async fn upsert_guild(&self, input: UpsertGuild<'_>) -> Result<GuildRecord, StorageError>;
    async fn get_guild(&self, guild_id: &str) -> Result<Option<GuildRecord>, StorageError>;
}

#[async_trait]
pub trait ModuleConfigurationRepository: Send + Sync {
    async fn get_module_configuration(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> Result<Option<ModuleConfiguration>, StorageError>;

    async fn update_module_configuration(
        &self,
        input: UpdateModuleConfiguration<'_>,
    ) -> Result<ModuleConfiguration, StorageError>;
}

impl Infrastructure {
    pub fn new(options: InfrastructureOptions<'_>) -> Result<Self, StorageError> {
        if options.database_max_connections == 0 {
            return Err(StorageError::Validation(
                "database_max_connections must be greater than zero",
            ));
        }
        if options.database_acquire_timeout.is_zero() {
            return Err(StorageError::Validation(
                "database_acquire_timeout must be greater than zero",
            ));
        }
        if options.dependency_timeout.is_zero() {
            return Err(StorageError::Validation(
                "dependency_timeout must be greater than zero",
            ));
        }

        let postgres = PgPoolOptions::new()
            .max_connections(options.database_max_connections)
            .acquire_timeout(options.database_acquire_timeout)
            .connect_lazy(options.database_url)?;
        let redis = redis::Client::open(options.redis_url)?;

        Ok(Self {
            postgres,
            redis,
            dependency_timeout: options.dependency_timeout,
        })
    }

    pub async fn migrate(&self) -> Result<(), StorageError> {
        MIGRATOR.run(&self.postgres).await?;
        Ok(())
    }

    pub async fn readiness(&self) -> ReadinessReport {
        let (postgres, redis) = tokio::join!(self.check_postgres(), self.check_redis());
        let status = if postgres.state == DependencyState::Ok
            && redis.state == DependencyState::Ok
        {
            ReadinessState::Ready
        } else {
            ReadinessState::NotReady
        };

        ReadinessReport {
            status,
            postgres,
            redis,
        }
    }

    async fn check_postgres(&self) -> DependencyCheck {
        let started = Instant::now();
        let check = timeout(
            self.dependency_timeout,
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&self.postgres),
        )
        .await;

        match check {
            Ok(Ok(1)) => DependencyCheck::ok(started),
            Ok(Ok(_)) | Ok(Err(_)) => DependencyCheck::unavailable(started),
            Err(_) => DependencyCheck::timed_out(started),
        }
    }

    async fn check_redis(&self) -> DependencyCheck {
        let started = Instant::now();
        let check = timeout(self.dependency_timeout, async {
            let mut connection = self.redis.get_multiplexed_async_connection().await?;
            let response: String = redis::cmd("PING").query_async(&mut connection).await?;
            Ok::<String, redis::RedisError>(response)
        })
        .await;

        match check {
            Ok(Ok(response)) if response == "PONG" => DependencyCheck::ok(started),
            Ok(Ok(_)) | Ok(Err(_)) => DependencyCheck::unavailable(started),
            Err(_) => DependencyCheck::timed_out(started),
        }
    }
}

impl DependencyCheck {
    fn ok(started: Instant) -> Self {
        Self {
            state: DependencyState::Ok,
            latency_ms: elapsed_milliseconds(started),
        }
    }

    fn unavailable(started: Instant) -> Self {
        Self {
            state: DependencyState::Unavailable,
            latency_ms: elapsed_milliseconds(started),
        }
    }

    fn timed_out(started: Instant) -> Self {
        Self {
            state: DependencyState::Timeout,
            latency_ms: elapsed_milliseconds(started),
        }
    }
}

fn elapsed_milliseconds(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[async_trait]
impl GuildRepository for Infrastructure {
    async fn upsert_guild(&self, input: UpsertGuild<'_>) -> Result<GuildRecord, StorageError> {
        validate_snowflake(input.guild_id)?;
        validate_locale(input.locale)?;
        if input.name.is_some_and(|name| name.chars().count() > 100) {
            return Err(StorageError::Validation(
                "guild name must not exceed 100 characters",
            ));
        }

        sqlx::query_as::<_, GuildRecord>(
            r#"
            INSERT INTO guilds (guild_id, name, locale)
            VALUES ($1, $2, $3)
            ON CONFLICT (guild_id) DO UPDATE
            SET name = EXCLUDED.name,
                locale = EXCLUDED.locale,
                version = guilds.version + 1,
                updated_at = NOW()
            RETURNING guild_id, name, locale, version, created_at, updated_at
            "#,
        )
        .bind(input.guild_id)
        .bind(input.name)
        .bind(input.locale)
        .fetch_one(&self.postgres)
        .await
        .map_err(StorageError::from)
    }

    async fn get_guild(&self, guild_id: &str) -> Result<Option<GuildRecord>, StorageError> {
        validate_snowflake(guild_id)?;
        sqlx::query_as::<_, GuildRecord>(
            r#"
            SELECT guild_id, name, locale, version, created_at, updated_at
            FROM guilds
            WHERE guild_id = $1
            "#,
        )
        .bind(guild_id)
        .fetch_optional(&self.postgres)
        .await
        .map_err(StorageError::from)
    }
}

#[async_trait]
impl ModuleConfigurationRepository for Infrastructure {
    async fn get_module_configuration(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> Result<Option<ModuleConfiguration>, StorageError> {
        validate_snowflake(guild_id)?;
        validate_module_id(module_id)?;

        sqlx::query_as::<_, ModuleConfiguration>(
            r#"
            SELECT guild_id, module_id, enabled, configuration, version, updated_by, updated_at
            FROM guild_modules
            WHERE guild_id = $1 AND module_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .fetch_optional(&self.postgres)
        .await
        .map_err(StorageError::from)
    }

    async fn update_module_configuration(
        &self,
        input: UpdateModuleConfiguration<'_>,
    ) -> Result<ModuleConfiguration, StorageError> {
        validate_snowflake(input.guild_id)?;
        validate_module_id(input.module_id)?;
        if input.expected_version < 0 {
            return Err(StorageError::Validation(
                "expected_version must not be negative",
            ));
        }
        if input.updated_by.trim().is_empty() || input.updated_by.chars().count() > 100 {
            return Err(StorageError::Validation(
                "updated_by must contain between 1 and 100 characters",
            ));
        }
        if serde_json::to_vec(input.configuration)
            .map_err(|_| StorageError::Validation("configuration must be valid JSON"))?
            .len()
            > MAX_CONFIGURATION_BYTES
        {
            return Err(StorageError::Validation(
                "configuration exceeds the 64 KiB limit",
            ));
        }

        let updated = if input.expected_version == 0 {
            sqlx::query_as::<_, ModuleConfiguration>(
                r#"
                INSERT INTO guild_modules (
                    guild_id, module_id, enabled, configuration, version, updated_by, updated_at
                )
                VALUES ($1, $2, $3, $4, 1, $5, NOW())
                ON CONFLICT (guild_id, module_id) DO NOTHING
                RETURNING guild_id, module_id, enabled, configuration, version, updated_by, updated_at
                "#,
            )
            .bind(input.guild_id)
            .bind(input.module_id)
            .bind(input.enabled)
            .bind(input.configuration)
            .bind(input.updated_by)
            .fetch_optional(&self.postgres)
            .await?
        } else {
            sqlx::query_as::<_, ModuleConfiguration>(
                r#"
                UPDATE guild_modules
                SET enabled = $3,
                    configuration = $4,
                    version = version + 1,
                    updated_by = $5,
                    updated_at = NOW()
                WHERE guild_id = $1 AND module_id = $2 AND version = $6
                RETURNING guild_id, module_id, enabled, configuration, version, updated_by, updated_at
                "#,
            )
            .bind(input.guild_id)
            .bind(input.module_id)
            .bind(input.enabled)
            .bind(input.configuration)
            .bind(input.updated_by)
            .bind(input.expected_version)
            .fetch_optional(&self.postgres)
            .await?
        };

        if let Some(configuration) = updated {
            return Ok(configuration);
        }

        let current_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM guild_modules WHERE guild_id = $1 AND module_id = $2",
        )
        .bind(input.guild_id)
        .bind(input.module_id)
        .fetch_optional(&self.postgres)
        .await?;

        Err(StorageError::VersionConflict { current_version })
    }
}

fn validate_snowflake(value: &str) -> Result<(), StorageError> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(StorageError::Validation(
            "Discord snowflake must contain between 1 and 20 ASCII digits",
        ));
    }
    Ok(())
}

fn validate_locale(value: &str) -> Result<(), StorageError> {
    if value.is_empty()
        || value.len() > 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(StorageError::Validation("locale has an invalid format"));
    }
    Ok(())
}

fn validate_module_id(value: &str) -> Result<(), StorageError> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        return Err(StorageError::Validation(
            "module_id must contain lowercase ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_numeric_discord_identifiers() {
        assert!(matches!(
            validate_snowflake("guild-one"),
            Err(StorageError::Validation(_))
        ));
    }

    #[test]
    fn accepts_namespaced_module_identifiers() {
        assert!(validate_module_id("moderation-v2").is_ok());
        assert!(validate_module_id("music_queue").is_ok());
    }

    #[test]
    fn infrastructure_debug_output_redacts_credentials() {
        let options = InfrastructureOptions {
            database_url: "postgres://user:secret@example/database",
            redis_url: "redis://:secret@example",
            database_max_connections: 5,
            database_acquire_timeout: Duration::from_secs(1),
            dependency_timeout: Duration::from_secs(1),
        };

        let debug = format!("{options:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("[REDACTED]"));
    }
}

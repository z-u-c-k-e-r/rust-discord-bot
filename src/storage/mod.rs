mod memory;
mod postgres;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use memory::MemoryStore;
pub use postgres::PostgresStore;

#[derive(Clone)]
pub enum Storage {
    Memory(MemoryStore),
    Postgres(PostgresStore),
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuildModuleSettings {
    pub guild_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModerationCase {
    pub id: i64,
    pub guild_id: String,
    pub target_user_id: String,
    pub moderator_user_id: String,
    pub action: String,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: Value,
    pub points: i32,
    pub status: String,
    pub resolution: Option<String>,
    pub resolved_by_user_id: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Storage {
    pub async fn connect(database_url: Option<&str>) -> anyhow::Result<Self> {
        match database_url {
            Some(url) => Ok(Self::Postgres(PostgresStore::connect(url).await?)),
            None => {
                tracing::warn!(
                    "DATABASE_URL is not set; using volatile in-memory configuration storage"
                );
                Ok(Self::Memory(MemoryStore::default()))
            }
        }
    }

    pub async fn get_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> anyhow::Result<Option<GuildModuleSettings>> {
        match self {
            Self::Memory(store) => Ok(store.get_module_settings(guild_id, module_id)),
            Self::Postgres(store) => store.get_module_settings(guild_id, module_id).await,
        }
    }

    pub async fn set_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
        enabled: bool,
        config: Value,
    ) -> anyhow::Result<GuildModuleSettings> {
        match self {
            Self::Memory(store) => {
                Ok(store.set_module_settings(guild_id, module_id, enabled, config))
            }
            Self::Postgres(store) => {
                store
                    .set_module_settings(guild_id, module_id, enabled, config)
                    .await
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_moderation_case(
        &self,
        guild_id: &str,
        target_user_id: &str,
        moderator_user_id: &str,
        action: &str,
        reason: &str,
        expires_at: Option<DateTime<Utc>>,
        metadata: Value,
        points: i32,
    ) -> anyhow::Result<ModerationCase> {
        match self {
            Self::Memory(store) => Ok(store.create_moderation_case(
                guild_id,
                target_user_id,
                moderator_user_id,
                action,
                reason,
                expires_at,
                metadata,
                points,
            )),
            Self::Postgres(store) => {
                store
                    .create_moderation_case(
                        guild_id,
                        target_user_id,
                        moderator_user_id,
                        action,
                        reason,
                        expires_at,
                        metadata,
                        points,
                    )
                    .await
            }
        }
    }

    pub async fn list_moderation_cases(
        &self,
        guild_id: &str,
        target_user_id: &str,
        include_resolved: bool,
        limit: u8,
    ) -> anyhow::Result<Vec<ModerationCase>> {
        match self {
            Self::Memory(store) => {
                Ok(store.list_moderation_cases(guild_id, target_user_id, include_resolved, limit))
            }
            Self::Postgres(store) => {
                store
                    .list_moderation_cases(guild_id, target_user_id, include_resolved, limit)
                    .await
            }
        }
    }

    pub async fn resolve_moderation_case(
        &self,
        guild_id: &str,
        case_id: i64,
        resolved_by_user_id: &str,
        resolution: &str,
    ) -> anyhow::Result<Option<ModerationCase>> {
        match self {
            Self::Memory(store) => Ok(store.resolve_moderation_case(
                guild_id,
                case_id,
                resolved_by_user_id,
                resolution,
            )),
            Self::Postgres(store) => {
                store
                    .resolve_moderation_case(guild_id, case_id, resolved_by_user_id, resolution)
                    .await
            }
        }
    }

    pub async fn active_moderation_points(
        &self,
        guild_id: &str,
        target_user_id: &str,
    ) -> anyhow::Result<i64> {
        match self {
            Self::Memory(store) => Ok(store.active_moderation_points(guild_id, target_user_id)),
            Self::Postgres(store) => {
                store
                    .active_moderation_points(guild_id, target_user_id)
                    .await
            }
        }
    }

    pub async fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) -> anyhow::Result<()> {
        match self {
            Self::Memory(store) => {
                store.record_audit(guild_id, actor_id, module_id, event, data);
                Ok(())
            }
            Self::Postgres(store) => {
                store
                    .record_audit(guild_id, actor_id, module_id, event, data)
                    .await
            }
        }
    }
}

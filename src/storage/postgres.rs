use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};

use super::{GuildModuleSettings, ModerationCase};

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn get_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> anyhow::Result<Option<GuildModuleSettings>> {
        let result = sqlx::query_as::<_, GuildModuleSettings>(
            r#"
            SELECT guild_id, module_id, enabled, config, updated_at
            FROM guild_module_settings
            WHERE guild_id = $1 AND module_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn set_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
        enabled: bool,
        config: Value,
    ) -> anyhow::Result<GuildModuleSettings> {
        let result = sqlx::query_as::<_, GuildModuleSettings>(
            r#"
            INSERT INTO guild_module_settings (guild_id, module_id, enabled, config)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (guild_id, module_id)
            DO UPDATE SET
                enabled = EXCLUDED.enabled,
                config = EXCLUDED.config,
                updated_at = NOW()
            RETURNING guild_id, module_id, enabled, config, updated_at
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .bind(enabled)
        .bind(config)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
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
        let result = sqlx::query_as::<_, ModerationCase>(
            r#"
            INSERT INTO moderation_cases (
                guild_id,
                target_user_id,
                moderator_user_id,
                action,
                reason,
                expires_at,
                metadata,
                points
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id,
                guild_id,
                target_user_id,
                moderator_user_id,
                action,
                reason,
                expires_at,
                metadata,
                points,
                status,
                resolution,
                resolved_by_user_id,
                resolved_at,
                created_at
            "#,
        )
        .bind(guild_id)
        .bind(target_user_id)
        .bind(moderator_user_id)
        .bind(action)
        .bind(reason)
        .bind(expires_at)
        .bind(metadata)
        .bind(points)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn list_moderation_cases(
        &self,
        guild_id: &str,
        target_user_id: &str,
        include_resolved: bool,
        limit: u8,
    ) -> anyhow::Result<Vec<ModerationCase>> {
        let result = sqlx::query_as::<_, ModerationCase>(
            r#"
            SELECT
                id,
                guild_id,
                target_user_id,
                moderator_user_id,
                action,
                reason,
                expires_at,
                metadata,
                points,
                status,
                resolution,
                resolved_by_user_id,
                resolved_at,
                created_at
            FROM moderation_cases
            WHERE guild_id = $1
                AND target_user_id = $2
                AND ($3 OR status = 'open')
            ORDER BY created_at DESC
            LIMIT $4
            "#,
        )
        .bind(guild_id)
        .bind(target_user_id)
        .bind(include_resolved)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn resolve_moderation_case(
        &self,
        guild_id: &str,
        case_id: i64,
        resolved_by_user_id: &str,
        resolution: &str,
    ) -> anyhow::Result<Option<ModerationCase>> {
        let result = sqlx::query_as::<_, ModerationCase>(
            r#"
            UPDATE moderation_cases
            SET
                status = 'resolved',
                resolution = $4,
                resolved_by_user_id = $3,
                resolved_at = NOW()
            WHERE guild_id = $1
                AND id = $2
                AND status = 'open'
            RETURNING
                id,
                guild_id,
                target_user_id,
                moderator_user_id,
                action,
                reason,
                expires_at,
                metadata,
                points,
                status,
                resolution,
                resolved_by_user_id,
                resolved_at,
                created_at
            "#,
        )
        .bind(guild_id)
        .bind(case_id)
        .bind(resolved_by_user_id)
        .bind(resolution)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn active_moderation_points(
        &self,
        guild_id: &str,
        target_user_id: &str,
    ) -> anyhow::Result<i64> {
        let points = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COALESCE(SUM(points), 0)::BIGINT
            FROM moderation_cases
            WHERE guild_id = $1
                AND target_user_id = $2
                AND status = 'open'
                AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(guild_id)
        .bind(target_user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(points)
    }

    pub async fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_events (guild_id, actor_id, module_id, event, data)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(guild_id)
        .bind(actor_id)
        .bind(module_id)
        .bind(event)
        .bind(data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

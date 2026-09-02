use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};

use super::GuildModuleSettings;

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

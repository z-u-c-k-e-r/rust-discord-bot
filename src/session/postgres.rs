use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};

use super::model::{DashboardGuild, DashboardUser, Session, hash_session_id};

#[derive(sqlx::FromRow)]
struct SessionRow {
    user_data: serde_json::Value,
    guilds_data: serde_json::Value,
    csrf_token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PostgresSessionStore {
    pool: PgPool,
}

impl PostgresSessionStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn save_session(
        &self,
        session_id: &str,
        user: DashboardUser,
        guilds: Vec<DashboardGuild>,
        csrf_token: String,
        expires_at: DateTime<Utc>,
        _access_token: Option<String>,
    ) -> anyhow::Result<()> {
        let hash = hash_session_id(session_id);
        let user_id = user.id.clone();
        let user_data = serde_json::to_value(&user)?;
        let guilds_data = serde_json::to_value(&guilds)?;

        sqlx::query(
            r#"
            INSERT INTO dashboard_sessions (
                session_hash, user_id, user_data, guilds_data, csrf_token, expires_at, created_at, last_seen_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (session_hash) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                user_data = EXCLUDED.user_data,
                guilds_data = EXCLUDED.guilds_data,
                csrf_token = EXCLUDED.csrf_token,
                expires_at = EXCLUDED.expires_at,
                last_seen_at = NOW()
            "#,
        )
        .bind(hash)
        .bind(user_id)
        .bind(user_data)
        .bind(guilds_data)
        .bind(csrf_token)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        let hash = hash_session_id(session_id);
        let record = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT user_data, guilds_data, csrf_token, expires_at
            FROM dashboard_sessions
            WHERE session_hash = $1
            "#,
        )
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = record else {
            return Ok(None);
        };

        if row.expires_at <= Utc::now() {
            let _ = self.delete_session(session_id).await;
            return Ok(None);
        }

        let pool = self.pool.clone();
        let h = hash.clone();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "UPDATE dashboard_sessions SET last_seen_at = NOW() WHERE session_hash = $1",
            )
            .bind(h)
            .execute(&pool)
            .await;
        });

        let user: DashboardUser = serde_json::from_value(row.user_data)?;
        let guilds: Vec<DashboardGuild> = serde_json::from_value(row.guilds_data)?;

        Ok(Some(Session {
            user,
            guilds,
            csrf_token: row.csrf_token,
            expires_at: row.expires_at,
            access_token: None, // Discord token not retained in persistent session
        }))
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let hash = hash_session_id(session_id);
        sqlx::query("DELETE FROM dashboard_sessions WHERE session_hash = $1")
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM dashboard_sessions WHERE expires_at <= NOW()")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn save_oauth_state(&self, state: &str) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO dashboard_oauth_states (state_token, created_at)
            VALUES ($1, NOW())
            ON CONFLICT (state_token) DO UPDATE SET created_at = NOW()
            "#,
        )
        .bind(state)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_oauth_state(&self, state: &str) -> anyhow::Result<bool> {
        let row = sqlx::query_scalar::<_, DateTime<Utc>>(
            r#"
            DELETE FROM dashboard_oauth_states
            WHERE state_token = $1
            RETURNING created_at
            "#,
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(created_at) = row {
            let valid = Utc::now() - created_at <= Duration::minutes(10);
            return Ok(valid);
        }
        Ok(false)
    }

    pub async fn cleanup_expired_oauth_states(&self) -> anyhow::Result<u64> {
        let result = sqlx::query(
            "DELETE FROM dashboard_oauth_states WHERE created_at < NOW() - INTERVAL '10 minutes'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn count_active_sessions(&self) -> anyhow::Result<usize> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM dashboard_sessions WHERE expires_at > NOW()",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count as usize)
    }

    pub async fn count_pending_oauth_states(&self) -> anyhow::Result<usize> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM dashboard_oauth_states WHERE created_at >= NOW() - INTERVAL '10 minutes'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count as usize)
    }
}

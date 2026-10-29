mod memory;
mod model;
mod postgres;

use anyhow::Result;
use chrono::DateTime;

pub use memory::MemorySessionStore;
pub use model::{DashboardGuild, DashboardUser, Session, StoredSession, hash_session_id};
pub use postgres::PostgresSessionStore;

#[derive(Clone)]
pub enum SessionStore {
    Memory(MemorySessionStore),
    Postgres(PostgresSessionStore),
}

impl SessionStore {
    pub async fn connect(database_url: Option<&str>) -> Result<Self> {
        match database_url {
            Some(url) => Ok(Self::Postgres(PostgresSessionStore::connect(url).await?)),
            None => {
                tracing::warn!(
                    "DATABASE_URL is not set; dashboard sessions are volatile and stored in-memory"
                );
                Ok(Self::Memory(MemorySessionStore::default()))
            }
        }
    }

    #[must_use]
    pub fn memory() -> Self {
        Self::Memory(MemorySessionStore::default())
    }

    pub async fn save_session(
        &self,
        session_id: &str,
        user: DashboardUser,
        guilds: Vec<DashboardGuild>,
        csrf_token: String,
        expires_at: DateTime<chrono::Utc>,
        access_token: Option<String>,
    ) -> Result<()> {
        match self {
            Self::Memory(store) => store.save_session(
                session_id,
                user,
                guilds,
                csrf_token,
                expires_at,
                access_token,
            ),
            Self::Postgres(store) => {
                store
                    .save_session(
                        session_id,
                        user,
                        guilds,
                        csrf_token,
                        expires_at,
                        access_token,
                    )
                    .await
            }
        }
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        match self {
            Self::Memory(store) => store.get_session(session_id),
            Self::Postgres(store) => store.get_session(session_id).await,
        }
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        match self {
            Self::Memory(store) => store.delete_session(session_id),
            Self::Postgres(store) => store.delete_session(session_id).await,
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        match self {
            Self::Memory(store) => store.cleanup_expired_sessions(),
            Self::Postgres(store) => store.cleanup_expired_sessions().await,
        }
    }

    pub async fn save_oauth_state(&self, state: &str) -> Result<()> {
        match self {
            Self::Memory(store) => store.save_oauth_state(state),
            Self::Postgres(store) => store.save_oauth_state(state).await,
        }
    }

    pub async fn consume_oauth_state(&self, state: &str) -> Result<bool> {
        match self {
            Self::Memory(store) => store.consume_oauth_state(state),
            Self::Postgres(store) => store.consume_oauth_state(state).await,
        }
    }

    pub async fn cleanup_expired_oauth_states(&self) -> Result<u64> {
        match self {
            Self::Memory(store) => store.cleanup_expired_oauth_states(),
            Self::Postgres(store) => store.cleanup_expired_oauth_states().await,
        }
    }

    pub async fn count_active_sessions(&self) -> Result<usize> {
        match self {
            Self::Memory(store) => store.count_active_sessions(),
            Self::Postgres(store) => store.count_active_sessions().await,
        }
    }

    pub async fn count_pending_oauth_states(&self) -> Result<usize> {
        match self {
            Self::Memory(store) => store.count_pending_oauth_states(),
            Self::Postgres(store) => store.count_pending_oauth_states().await,
        }
    }
}

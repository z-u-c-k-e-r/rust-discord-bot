use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;

use super::model::{DashboardGuild, DashboardUser, Session, StoredSession, hash_session_id};

#[derive(Clone, Default)]
pub struct MemorySessionStore {
    sessions: Arc<DashMap<String, StoredSession>>,
    oauth_states: Arc<DashMap<String, DateTime<Utc>>>,
}

impl MemorySessionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_session(
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
        let now = Utc::now();
        self.sessions.insert(
            hash.clone(),
            StoredSession {
                session_hash: hash,
                user_id,
                user_data,
                guilds_data,
                csrf_token,
                expires_at,
                created_at: now,
                last_seen_at: now,
            },
        );
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        let hash = hash_session_id(session_id);
        let now = Utc::now();
        if let Some(mut entry) = self.sessions.get_mut(&hash) {
            if entry.expires_at <= now {
                drop(entry);
                self.sessions.remove(&hash);
                return Ok(None);
            }
            entry.last_seen_at = now;
            let user: DashboardUser = serde_json::from_value(entry.user_data.clone())?;
            let guilds: Vec<DashboardGuild> = serde_json::from_value(entry.guilds_data.clone())?;
            return Ok(Some(Session {
                user,
                guilds,
                csrf_token: entry.csrf_token.clone(),
                expires_at: entry.expires_at,
                access_token: None, // Discord token not retained in persistent session
            }));
        }
        Ok(None)
    }

    pub fn delete_session(&self, session_id: &str) -> anyhow::Result<()> {
        let hash = hash_session_id(session_id);
        self.sessions.remove(&hash);
        Ok(())
    }

    pub fn cleanup_expired_sessions(&self) -> anyhow::Result<u64> {
        let now = Utc::now();
        let before = self.sessions.len();
        self.sessions.retain(|_, s| s.expires_at > now);
        let after = self.sessions.len();
        Ok(before.saturating_sub(after) as u64)
    }

    pub fn save_oauth_state(&self, state: &str) -> anyhow::Result<()> {
        self.oauth_states.insert(state.to_owned(), Utc::now());
        Ok(())
    }

    pub fn consume_oauth_state(&self, state: &str) -> anyhow::Result<bool> {
        if let Some((_, created_at)) = self.oauth_states.remove(state) {
            let valid = Utc::now() - created_at <= Duration::minutes(10);
            return Ok(valid);
        }
        Ok(false)
    }

    pub fn cleanup_expired_oauth_states(&self) -> anyhow::Result<u64> {
        let cutoff = Utc::now() - Duration::minutes(10);
        let before = self.oauth_states.len();
        self.oauth_states
            .retain(|_, created_at| *created_at >= cutoff);
        let after = self.oauth_states.len();
        Ok(before.saturating_sub(after) as u64)
    }

    pub fn count_active_sessions(&self) -> anyhow::Result<usize> {
        let now = Utc::now();
        Ok(self.sessions.iter().filter(|s| s.expires_at > now).count())
    }

    pub fn count_pending_oauth_states(&self) -> anyhow::Result<usize> {
        let cutoff = Utc::now() - Duration::minutes(10);
        Ok(self
            .oauth_states
            .iter()
            .filter(|s| *s.value() >= cutoff)
            .count())
    }
}

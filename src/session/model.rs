use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner: bool,
    pub permissions: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub user: DashboardUser,
    pub guilds: Vec<DashboardGuild>,
    pub csrf_token: String,
    #[serde(skip)]
    pub expires_at: DateTime<Utc>,
    #[serde(skip)]
    pub access_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_hash: String,
    pub user_id: String,
    pub user_data: serde_json::Value,
    pub guilds_data: serde_json::Value,
    pub csrf_token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

pub fn hash_session_id(session_id: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(session_id.as_bytes());
    hex::encode(hasher.finalize())
}

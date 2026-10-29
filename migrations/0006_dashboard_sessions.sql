CREATE TABLE IF NOT EXISTS dashboard_sessions (
    session_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    user_data JSONB NOT NULL,
    guilds_data JSONB NOT NULL,
    csrf_token TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS dashboard_sessions_expires_at_idx
    ON dashboard_sessions (expires_at);

CREATE INDEX IF NOT EXISTS dashboard_sessions_user_id_idx
    ON dashboard_sessions (user_id);

CREATE TABLE IF NOT EXISTS dashboard_oauth_states (
    state_token TEXT PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS dashboard_oauth_states_created_at_idx
    ON dashboard_oauth_states (created_at);

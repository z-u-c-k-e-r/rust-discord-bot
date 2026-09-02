ALTER TABLE guilds
    ADD COLUMN version BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT guilds_version_positive CHECK (version > 0);

ALTER TABLE guild_modules
    ADD COLUMN version BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT guild_modules_version_positive CHECK (version > 0);

ALTER TABLE audit_events
    ADD COLUMN outcome TEXT NOT NULL DEFAULT 'success',
    ADD COLUMN error_code TEXT;

CREATE TABLE web_sessions (
    session_id_hash BYTEA PRIMARY KEY,
    user_id TEXT NOT NULL,
    state JSONB NOT NULL DEFAULT '{}'::jsonb,
    csrf_token_hash BYTEA NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX web_sessions_user_expires_idx
    ON web_sessions (user_id, expires_at DESC);

CREATE INDEX web_sessions_expiry_idx
    ON web_sessions (expires_at);

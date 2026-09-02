ALTER TABLE guild_modules
    ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;

ALTER TABLE audit_events
    ADD COLUMN IF NOT EXISTS outcome TEXT NOT NULL DEFAULT 'success';

ALTER TABLE audit_events
    ADD COLUMN IF NOT EXISTS error_code TEXT;

ALTER TABLE audit_events
    ADD COLUMN IF NOT EXISTS metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX IF NOT EXISTS guild_modules_updated_idx
    ON guild_modules (guild_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS audit_events_request_idx
    ON audit_events (request_id)
    WHERE request_id IS NOT NULL;

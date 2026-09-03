ALTER TABLE moderation_cases
    ADD COLUMN IF NOT EXISTS points INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'open',
    ADD COLUMN IF NOT EXISTS resolution TEXT,
    ADD COLUMN IF NOT EXISTS resolved_by_user_id TEXT,
    ADD COLUMN IF NOT EXISTS resolved_at TIMESTAMPTZ;

ALTER TABLE moderation_cases
    DROP CONSTRAINT IF EXISTS moderation_cases_points_check;

ALTER TABLE moderation_cases
    ADD CONSTRAINT moderation_cases_points_check
    CHECK (points BETWEEN 0 AND 1000);

ALTER TABLE moderation_cases
    DROP CONSTRAINT IF EXISTS moderation_cases_status_check;

ALTER TABLE moderation_cases
    ADD CONSTRAINT moderation_cases_status_check
    CHECK (status IN ('open', 'resolved', 'void'));

CREATE INDEX IF NOT EXISTS moderation_cases_open_target_idx
    ON moderation_cases (guild_id, target_user_id, created_at DESC)
    WHERE status = 'open';

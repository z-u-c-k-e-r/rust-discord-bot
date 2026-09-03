ALTER TABLE scheduled_jobs
    ALTER COLUMN payload SET DEFAULT '{}'::JSONB,
    ADD COLUMN channel_id TEXT,
    ADD COLUMN creator_user_id TEXT,
    ADD COLUMN content TEXT,
    ADD COLUMN mention_creator BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN repeat_every_seconds BIGINT,
    ADD COLUMN remaining_runs BIGINT,
    ADD COLUMN run_count BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN status TEXT NOT NULL DEFAULT 'active',
    ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 5,
    ADD COLUMN locked_by TEXT,
    ADD COLUMN last_error TEXT,
    ADD COLUMN last_run_at TIMESTAMPTZ,
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE scheduled_jobs
SET channel_id = COALESCE(payload ->> 'channel_id', '0'),
    creator_user_id = COALESCE(payload ->> 'creator_user_id', '0'),
    content = COALESCE(payload ->> 'content', 'Legacy scheduled job requires manual migration'),
    status = CASE
        WHEN payload ? 'channel_id' AND payload ? 'creator_user_id' AND payload ? 'content'
            THEN status
        ELSE 'failed'
    END,
    last_error = CASE
        WHEN payload ? 'channel_id' AND payload ? 'creator_user_id' AND payload ? 'content'
            THEN last_error
        ELSE 'Legacy scheduled job did not contain the v2 scheduler payload'
    END,
    completed_at = CASE
        WHEN payload ? 'channel_id' AND payload ? 'creator_user_id' AND payload ? 'content'
            THEN completed_at
        ELSE NOW()
    END;

ALTER TABLE scheduled_jobs
    ALTER COLUMN channel_id SET NOT NULL,
    ALTER COLUMN creator_user_id SET NOT NULL,
    ALTER COLUMN content SET NOT NULL,
    ADD CONSTRAINT scheduled_jobs_status_check
        CHECK (status IN ('active', 'paused', 'processing', 'completed', 'cancelled', 'failed')),
    ADD CONSTRAINT scheduled_jobs_content_check
        CHECK (char_length(content) BETWEEN 1 AND 1800),
    ADD CONSTRAINT scheduled_jobs_repeat_check
        CHECK (repeat_every_seconds IS NULL OR repeat_every_seconds BETWEEN 60 AND 31557600),
    ADD CONSTRAINT scheduled_jobs_remaining_runs_check
        CHECK (remaining_runs IS NULL OR remaining_runs >= 0),
    ADD CONSTRAINT scheduled_jobs_run_count_check
        CHECK (run_count >= 0),
    ADD CONSTRAINT scheduled_jobs_attempts_check
        CHECK (attempts >= 0),
    ADD CONSTRAINT scheduled_jobs_max_attempts_check
        CHECK (max_attempts BETWEEN 1 AND 20);

DROP INDEX IF EXISTS scheduled_jobs_due_idx;

CREATE INDEX scheduled_jobs_due_idx
    ON scheduled_jobs (run_at, id)
    WHERE status IN ('active', 'processing');

CREATE INDEX scheduled_jobs_creator_pending_idx
    ON scheduled_jobs (guild_id, creator_user_id, run_at)
    WHERE status IN ('active', 'paused', 'processing');

CREATE INDEX scheduled_jobs_locked_idx
    ON scheduled_jobs (locked_at)
    WHERE status = 'processing';

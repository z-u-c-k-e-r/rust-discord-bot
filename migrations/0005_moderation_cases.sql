CREATE TABLE IF NOT EXISTS moderation_case_counters (
    guild_id TEXT PRIMARY KEY,
    next_case_number BIGINT NOT NULL CHECK (next_case_number >= 1)
);

CREATE TABLE IF NOT EXISTS moderation_cases (
    id UUID PRIMARY KEY,
    case_number BIGINT NOT NULL CHECK (case_number >= 1),
    guild_id TEXT NOT NULL CHECK (guild_id ~ '^[1-9][0-9]{0,19}$'),
    subject_user_id TEXT NOT NULL CHECK (subject_user_id ~ '^[1-9][0-9]{0,19}$'),
    actor_user_id TEXT NOT NULL CHECK (actor_user_id ~ '^[1-9][0-9]{0,19}$'),
    kind TEXT NOT NULL CHECK (
        kind IN ('warning', 'staff_note', 'timeout', 'kick', 'ban', 'unban', 'automod', 'other')
    ),
    status TEXT NOT NULL CHECK (status IN ('active', 'expired', 'voided')),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
    points INTEGER NOT NULL DEFAULT 0 CHECK (points BETWEEN 0 AND 10000),
    reason TEXT NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 2000),
    source_module TEXT NOT NULL CHECK (char_length(source_module) BETWEEN 1 AND 64),
    visible_to_subject BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMPTZ,
    voided_by_user_id TEXT CHECK (
        voided_by_user_id IS NULL OR voided_by_user_id ~ '^[1-9][0-9]{0,19}$'
    ),
    void_reason TEXT CHECK (
        void_reason IS NULL OR char_length(void_reason) BETWEEN 1 AND 2000
    ),
    voided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    UNIQUE (guild_id, case_number),
    CHECK (
        (status = 'voided' AND voided_by_user_id IS NOT NULL AND void_reason IS NOT NULL AND voided_at IS NOT NULL)
        OR
        (status <> 'voided' AND voided_by_user_id IS NULL AND void_reason IS NULL AND voided_at IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS moderation_case_notes (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES moderation_cases(id) ON DELETE CASCADE,
    author_user_id TEXT NOT NULL CHECK (author_user_id ~ '^[1-9][0-9]{0,19}$'),
    body TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 2000),
    visible_to_subject BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS moderation_case_evidence (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES moderation_cases(id) ON DELETE CASCADE,
    author_user_id TEXT NOT NULL CHECK (author_user_id ~ '^[1-9][0-9]{0,19}$'),
    label TEXT NOT NULL CHECK (char_length(label) BETWEEN 1 AND 200),
    value TEXT NOT NULL CHECK (char_length(value) BETWEEN 1 AND 4096),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS moderation_case_events (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES moderation_cases(id) ON DELETE CASCADE,
    actor_user_id TEXT NOT NULL CHECK (actor_user_id ~ '^[1-9][0-9]{0,19}$'),
    event_type TEXT NOT NULL CHECK (char_length(event_type) BETWEEN 1 AND 64),
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS moderation_cases_guild_subject_number_idx
    ON moderation_cases (guild_id, subject_user_id, case_number DESC);
CREATE INDEX IF NOT EXISTS moderation_cases_guild_actor_created_idx
    ON moderation_cases (guild_id, actor_user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS moderation_cases_guild_status_expiry_idx
    ON moderation_cases (guild_id, status, expires_at)
    WHERE status = 'active' AND expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS moderation_case_notes_case_created_idx
    ON moderation_case_notes (case_id, created_at ASC);
CREATE INDEX IF NOT EXISTS moderation_case_evidence_case_created_idx
    ON moderation_case_evidence (case_id, created_at ASC);
CREATE INDEX IF NOT EXISTS moderation_case_events_case_created_idx
    ON moderation_case_events (case_id, created_at ASC);

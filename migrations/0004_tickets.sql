CREATE TABLE tickets (
    id UUID PRIMARY KEY,
    number BIGSERIAL NOT NULL UNIQUE,
    guild_id TEXT NOT NULL,
    creator_user_id TEXT NOT NULL,
    channel_id TEXT,
    channel_name TEXT,
    subject TEXT NOT NULL,
    description TEXT NOT NULL,
    queue TEXT NOT NULL,
    priority TEXT NOT NULL DEFAULT 'normal',
    status TEXT NOT NULL DEFAULT 'provisioning',
    claimed_by_user_id TEXT,
    first_response_at TIMESTAMPTZ,
    last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    close_reason TEXT,
    closed_by_user_id TEXT,
    provisioning_error TEXT,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    CONSTRAINT tickets_subject_check CHECK (char_length(subject) BETWEEN 1 AND 100),
    CONSTRAINT tickets_description_check CHECK (char_length(description) BETWEEN 1 AND 1800),
    CONSTRAINT tickets_queue_check CHECK (
        char_length(queue) BETWEEN 1 AND 32
        AND queue ~ '^[a-z0-9_-]+$'
    ),
    CONSTRAINT tickets_priority_check CHECK (priority IN ('low', 'normal', 'high', 'urgent')),
    CONSTRAINT tickets_status_check CHECK (
        status IN ('provisioning', 'open', 'claimed', 'closed', 'failed')
    ),
    CONSTRAINT tickets_channel_name_check CHECK (
        channel_name IS NULL OR char_length(channel_name) BETWEEN 2 AND 100
    ),
    CONSTRAINT tickets_close_reason_check CHECK (
        close_reason IS NULL OR char_length(close_reason) BETWEEN 1 AND 512
    ),
    CONSTRAINT tickets_version_check CHECK (version > 0),
    CONSTRAINT tickets_channel_state_check CHECK (
        (status = 'provisioning' AND channel_id IS NULL)
        OR (status = 'failed')
        OR (status IN ('open', 'claimed', 'closed') AND channel_id IS NOT NULL)
    ),
    CONSTRAINT tickets_claim_state_check CHECK (
        (status = 'claimed' AND claimed_by_user_id IS NOT NULL)
        OR (status <> 'claimed')
    )
);

CREATE UNIQUE INDEX tickets_guild_channel_unique_idx
    ON tickets (guild_id, channel_id)
    WHERE channel_id IS NOT NULL;

CREATE INDEX tickets_creator_active_idx
    ON tickets (guild_id, creator_user_id, created_at)
    WHERE status IN ('provisioning', 'open', 'claimed');

CREATE INDEX tickets_queue_open_idx
    ON tickets (guild_id, queue, priority, created_at)
    WHERE status IN ('open', 'claimed');

CREATE INDEX tickets_claimed_by_idx
    ON tickets (guild_id, claimed_by_user_id, updated_at DESC)
    WHERE status = 'claimed';

CREATE INDEX tickets_provisioning_idx
    ON tickets (created_at)
    WHERE status = 'provisioning';

CREATE TABLE ticket_participants (
    ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    added_by_user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (ticket_id, user_id)
);

CREATE INDEX ticket_participants_user_idx
    ON ticket_participants (user_id, created_at DESC);

CREATE TABLE ticket_transcripts (
    id UUID PRIMARY KEY,
    ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    guild_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    generated_by_user_id TEXT NOT NULL,
    message_count INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ticket_transcripts_message_count_check CHECK (message_count >= 0),
    CONSTRAINT ticket_transcripts_content_check CHECK (char_length(content) > 0)
);

CREATE INDEX ticket_transcripts_ticket_idx
    ON ticket_transcripts (ticket_id, created_at DESC);

CREATE TABLE ticket_events (
    id BIGSERIAL PRIMARY KEY,
    ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    guild_id TEXT NOT NULL,
    actor_user_id TEXT,
    event_type TEXT NOT NULL,
    data JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ticket_events_type_check CHECK (char_length(event_type) BETWEEN 1 AND 64)
);

CREATE INDEX ticket_events_ticket_idx
    ON ticket_events (ticket_id, id DESC);

CREATE INDEX ticket_events_guild_idx
    ON ticket_events (guild_id, created_at DESC);

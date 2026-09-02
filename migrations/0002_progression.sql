CREATE TABLE member_progress (
    guild_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    xp BIGINT NOT NULL DEFAULT 0 CHECK (xp >= 0),
    level INTEGER NOT NULL DEFAULT 0 CHECK (level >= 0),
    coins BIGINT NOT NULL DEFAULT 0 CHECK (coins >= 0),
    reputation BIGINT NOT NULL DEFAULT 0 CHECK (reputation >= 0),
    messages BIGINT NOT NULL DEFAULT 0 CHECK (messages >= 0),
    daily_streak INTEGER NOT NULL DEFAULT 0 CHECK (daily_streak >= 0),
    last_xp_at TIMESTAMPTZ,
    last_daily_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (guild_id, user_id)
);

CREATE INDEX member_progress_xp_leaderboard_idx
    ON member_progress (guild_id, xp DESC, user_id);

CREATE INDEX member_progress_coins_leaderboard_idx
    ON member_progress (guild_id, coins DESC, user_id);

CREATE INDEX member_progress_reputation_leaderboard_idx
    ON member_progress (guild_id, reputation DESC, user_id);

CREATE INDEX member_progress_messages_leaderboard_idx
    ON member_progress (guild_id, messages DESC, user_id);

CREATE TABLE reputation_cooldowns (
    guild_id TEXT NOT NULL,
    giver_user_id TEXT NOT NULL,
    target_user_id TEXT,
    last_given_at TIMESTAMPTZ,
    PRIMARY KEY (guild_id, giver_user_id)
);

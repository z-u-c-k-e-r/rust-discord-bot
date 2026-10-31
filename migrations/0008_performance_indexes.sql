-- Dodatkowe indeksy wydajnosciowe dla zapytan panelu i bota

-- Moderation cases: filtrowanie wg serwera i statusu / typu / czasu utworzenia
CREATE INDEX IF NOT EXISTS moderation_cases_guild_created_idx
    ON moderation_cases (guild_id, created_at DESC);

CREATE INDEX IF NOT EXISTS moderation_cases_guild_status_idx
    ON moderation_cases (guild_id, status);

CREATE INDEX IF NOT EXISTS moderation_cases_guild_kind_idx
    ON moderation_cases (guild_id, kind);

-- Tickets: lista zgloszen per serwer i status
CREATE INDEX IF NOT EXISTS tickets_guild_status_created_idx
    ON tickets (guild_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS tickets_guild_creator_idx
    ON tickets (guild_id, creator_user_id);

-- Czesciowe indeksy dla rankingu aktywnosci uzytkownikow (omijaja profile z 0 XP / coins)
CREATE INDEX IF NOT EXISTS member_progress_xp_active_idx
    ON member_progress (guild_id, xp DESC, user_id)
    WHERE xp > 0;

CREATE INDEX IF NOT EXISTS member_progress_coins_active_idx
    ON member_progress (guild_id, coins DESC, user_id)
    WHERE coins > 0;

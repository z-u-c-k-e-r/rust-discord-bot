ALTER TABLE moderation_cases DROP CONSTRAINT IF EXISTS moderation_cases_kind_check;
ALTER TABLE moderation_cases ADD CONSTRAINT moderation_cases_kind_check CHECK (
    kind IN ('warning', 'staff_note', 'timeout', 'kick', 'ban', 'unban', 'automod', 'softban', 'other')
);

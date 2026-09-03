from pathlib import Path

path = Path("src/scheduler/postgres.rs")
content = path.read_text(encoding="utf-8")
start = content.index("    pub async fn list_jobs(")
end = content.index("\n    pub async fn mutate_job(", start)

replacement = r'''    pub async fn list_jobs(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> anyhow::Result<Vec<ScheduledJob>> {
        if include_all {
            Ok(sqlx::query_as::<_, ScheduledJob>(
                r#"
                SELECT
                    id::text AS id,
                    guild_id,
                    module_id,
                    channel_id,
                    creator_user_id,
                    content,
                    mention_creator,
                    run_at,
                    repeat_every_seconds,
                    remaining_runs,
                    run_count,
                    status,
                    attempts,
                    max_attempts,
                    locked_at,
                    locked_by,
                    last_error,
                    last_run_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM scheduled_jobs
                WHERE guild_id = $1
                  AND status IN ('active', 'paused', 'processing')
                ORDER BY run_at ASC, id ASC
                LIMIT $2
                "#,
            )
            .bind(guild_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
        } else {
            Ok(sqlx::query_as::<_, ScheduledJob>(
                r#"
                SELECT
                    id::text AS id,
                    guild_id,
                    module_id,
                    channel_id,
                    creator_user_id,
                    content,
                    mention_creator,
                    run_at,
                    repeat_every_seconds,
                    remaining_runs,
                    run_count,
                    status,
                    attempts,
                    max_attempts,
                    locked_at,
                    locked_by,
                    last_error,
                    last_run_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM scheduled_jobs
                WHERE guild_id = $1
                  AND creator_user_id = $2
                  AND status IN ('active', 'paused', 'processing')
                ORDER BY run_at ASC, id ASC
                LIMIT $3
                "#,
            )
            .bind(guild_id)
            .bind(creator_user_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
        }
    }
'''

path.write_text(content[:start] + replacement + content[end:], encoding="utf-8")

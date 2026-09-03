use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use uuid::Uuid;

use super::model::{
    CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, STATUS_ACTIVE,
    STATUS_CANCELLED, STATUS_PAUSED, STATUS_PROCESSING, ScheduledJob,
};

#[derive(Clone)]
pub struct PostgresSchedulerStore {
    pool: PgPool,
}

impl PostgresSchedulerStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn create_job(
        &self,
        new_job: NewScheduledJob,
        max_jobs: u16,
    ) -> anyhow::Result<CreateJobOutcome> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1), hashtext($2))")
            .bind(&new_job.guild_id)
            .bind(&new_job.creator_user_id)
            .fetch_optional(&mut *transaction)
            .await?;

        let pending_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM scheduled_jobs
            WHERE guild_id = $1
              AND creator_user_id = $2
              AND status IN ('active', 'paused', 'processing')
            "#,
        )
        .bind(&new_job.guild_id)
        .bind(&new_job.creator_user_id)
        .fetch_one(&mut *transaction)
        .await?;

        if pending_count >= i64::from(max_jobs) {
            transaction.commit().await?;
            return Ok(CreateJobOutcome::LimitReached { limit: max_jobs });
        }

        let job = new_job.materialize(Utc::now());
        let created = sqlx::query_as::<_, ScheduledJob>(
            r#"
            INSERT INTO scheduled_jobs (
                id,
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
                payload,
                created_at,
                updated_at
            )
            VALUES (
                $1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                0, 'active', 0, $11, $12, $13, $13
            )
            RETURNING
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
            "#,
        )
        .bind(&job.id)
        .bind(&job.guild_id)
        .bind(&job.module_id)
        .bind(&job.channel_id)
        .bind(&job.creator_user_id)
        .bind(&job.content)
        .bind(job.mention_creator)
        .bind(job.run_at)
        .bind(job.repeat_every_seconds)
        .bind(job.remaining_runs)
        .bind(job.max_attempts)
        .bind(json!({}))
        .bind(job.created_at)
        .fetch_one(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(CreateJobOutcome::Created(created))
    }

    pub async fn list_jobs(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> anyhow::Result<Vec<ScheduledJob>> {
        let common_tail = r#"
              AND status IN ('active', 'paused', 'processing')
            ORDER BY run_at ASC, id ASC
            LIMIT $3
        "#;
        let query = if include_all {
            format!(
                r#"
                SELECT
                    id::text AS id, guild_id, module_id, channel_id, creator_user_id,
                    content, mention_creator, run_at, repeat_every_seconds, remaining_runs,
                    run_count, status, attempts, max_attempts, locked_at, locked_by,
                    last_error, last_run_at, completed_at, created_at, updated_at
                FROM scheduled_jobs
                WHERE guild_id = $1
                  AND $2 = $2
                {common_tail}
                "#
            )
        } else {
            format!(
                r#"
                SELECT
                    id::text AS id, guild_id, module_id, channel_id, creator_user_id,
                    content, mention_creator, run_at, repeat_every_seconds, remaining_runs,
                    run_count, status, attempts, max_attempts, locked_at, locked_by,
                    last_error, last_run_at, completed_at, created_at, updated_at
                FROM scheduled_jobs
                WHERE guild_id = $1
                  AND creator_user_id = $2
                {common_tail}
                "#
            )
        };

        Ok(sqlx::query_as::<_, ScheduledJob>(&query)
            .bind(guild_id)
            .bind(creator_user_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn mutate_job(
        &self,
        guild_id: &str,
        job_id: &str,
        actor_user_id: &str,
        allow_any: bool,
        mutation: JobMutation,
    ) -> anyhow::Result<JobMutationOutcome> {
        if Uuid::parse_str(job_id).is_err() {
            return Ok(JobMutationOutcome::NotFound);
        }

        let mut transaction = self.pool.begin().await?;
        let Some(job) = fetch_job_for_update(&mut transaction, guild_id, job_id).await? else {
            transaction.commit().await?;
            return Ok(JobMutationOutcome::NotFound);
        };

        if !allow_any && job.creator_user_id != actor_user_id {
            transaction.commit().await?;
            return Ok(JobMutationOutcome::Forbidden);
        }

        let valid_state = match mutation {
            JobMutation::Cancel => job.is_pending(),
            JobMutation::Pause => job.status == STATUS_ACTIVE,
            JobMutation::Resume => job.status == STATUS_PAUSED,
        };
        if !valid_state {
            let current_status = job.status;
            transaction.commit().await?;
            return Ok(JobMutationOutcome::InvalidState { current_status });
        }

        match mutation {
            JobMutation::Cancel => {
                sqlx::query(
                    r#"
                    UPDATE scheduled_jobs
                    SET status = $3,
                        completed_at = NOW(),
                        locked_at = NULL,
                        locked_by = NULL,
                        updated_at = NOW()
                    WHERE guild_id = $1 AND id = $2::uuid
                    "#,
                )
                .bind(guild_id)
                .bind(job_id)
                .bind(STATUS_CANCELLED)
                .execute(&mut *transaction)
                .await?;
            }
            JobMutation::Pause => {
                sqlx::query(
                    r#"
                    UPDATE scheduled_jobs
                    SET status = $3,
                        locked_at = NULL,
                        locked_by = NULL,
                        updated_at = NOW()
                    WHERE guild_id = $1 AND id = $2::uuid
                    "#,
                )
                .bind(guild_id)
                .bind(job_id)
                .bind(STATUS_PAUSED)
                .execute(&mut *transaction)
                .await?;
            }
            JobMutation::Resume => {
                sqlx::query(
                    r#"
                    UPDATE scheduled_jobs
                    SET status = $3,
                        run_at = GREATEST(run_at, NOW() + INTERVAL '1 second'),
                        completed_at = NULL,
                        locked_at = NULL,
                        locked_by = NULL,
                        updated_at = NOW()
                    WHERE guild_id = $1 AND id = $2::uuid
                    "#,
                )
                .bind(guild_id)
                .bind(job_id)
                .bind(STATUS_ACTIVE)
                .execute(&mut *transaction)
                .await?;
            }
        }

        let updated = fetch_job_for_update(&mut transaction, guild_id, job_id)
            .await?
            .expect("updated scheduler job should still exist");
        transaction.commit().await?;
        Ok(JobMutationOutcome::Updated(updated))
    }

    pub async fn claim_due(
        &self,
        worker_id: &str,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
        limit: u8,
    ) -> anyhow::Result<Vec<ScheduledJob>> {
        Ok(sqlx::query_as::<_, ScheduledJob>(
            r#"
            WITH due AS (
                SELECT id
                FROM scheduled_jobs
                WHERE (status = 'active' AND run_at <= $2)
                   OR (status = 'processing' AND locked_at < $3)
                ORDER BY run_at ASC, id ASC
                FOR UPDATE SKIP LOCKED
                LIMIT $4
            )
            UPDATE scheduled_jobs AS jobs
            SET status = 'processing',
                locked_by = $1,
                locked_at = $2,
                attempts = jobs.attempts + 1,
                updated_at = $2
            FROM due
            WHERE jobs.id = due.id
            RETURNING
                jobs.id::text AS id,
                jobs.guild_id,
                jobs.module_id,
                jobs.channel_id,
                jobs.creator_user_id,
                jobs.content,
                jobs.mention_creator,
                jobs.run_at,
                jobs.repeat_every_seconds,
                jobs.remaining_runs,
                jobs.run_count,
                jobs.status,
                jobs.attempts,
                jobs.max_attempts,
                jobs.locked_at,
                jobs.locked_by,
                jobs.last_error,
                jobs.last_run_at,
                jobs.completed_at,
                jobs.created_at,
                jobs.updated_at
            "#,
        )
        .bind(worker_id)
        .bind(now)
        .bind(stale_before)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn mark_succeeded(
        &self,
        job_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
    ) -> anyhow::Result<Option<ScheduledJob>> {
        self.finish_attempt(job_id, worker_id, now, None).await
    }

    pub async fn mark_failed(
        &self,
        job_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        error: &str,
    ) -> anyhow::Result<Option<ScheduledJob>> {
        self.finish_attempt(job_id, worker_id, now, Some(error))
            .await
    }

    async fn finish_attempt(
        &self,
        job_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        error: Option<&str>,
    ) -> anyhow::Result<Option<ScheduledJob>> {
        if Uuid::parse_str(job_id).is_err() {
            return Ok(None);
        }

        let mut transaction = self.pool.begin().await?;
        let Some(mut job) = fetch_processing_job(&mut transaction, job_id, worker_id).await? else {
            transaction.commit().await?;
            return Ok(None);
        };

        match error {
            Some(error) => job.mark_failed(now, error),
            None => job.mark_succeeded(now),
        }
        write_runtime_state(&mut transaction, &job).await?;
        transaction.commit().await?;
        Ok(Some(job))
    }
}

async fn fetch_job_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    job_id: &str,
) -> Result<Option<ScheduledJob>, sqlx::Error> {
    sqlx::query_as::<_, ScheduledJob>(
        r#"
        SELECT
            id::text AS id, guild_id, module_id, channel_id, creator_user_id,
            content, mention_creator, run_at, repeat_every_seconds, remaining_runs,
            run_count, status, attempts, max_attempts, locked_at, locked_by,
            last_error, last_run_at, completed_at, created_at, updated_at
        FROM scheduled_jobs
        WHERE guild_id = $1 AND id = $2::uuid
        FOR UPDATE
        "#,
    )
    .bind(guild_id)
    .bind(job_id)
    .fetch_optional(&mut **transaction)
    .await
}

async fn fetch_processing_job(
    transaction: &mut Transaction<'_, Postgres>,
    job_id: &str,
    worker_id: &str,
) -> Result<Option<ScheduledJob>, sqlx::Error> {
    sqlx::query_as::<_, ScheduledJob>(
        r#"
        SELECT
            id::text AS id, guild_id, module_id, channel_id, creator_user_id,
            content, mention_creator, run_at, repeat_every_seconds, remaining_runs,
            run_count, status, attempts, max_attempts, locked_at, locked_by,
            last_error, last_run_at, completed_at, created_at, updated_at
        FROM scheduled_jobs
        WHERE id = $1::uuid
          AND status = $2
          AND locked_by = $3
        FOR UPDATE
        "#,
    )
    .bind(job_id)
    .bind(STATUS_PROCESSING)
    .bind(worker_id)
    .fetch_optional(&mut **transaction)
    .await
}

async fn write_runtime_state(
    transaction: &mut Transaction<'_, Postgres>,
    job: &ScheduledJob,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE scheduled_jobs
        SET run_at = $2,
            remaining_runs = $3,
            run_count = $4,
            status = $5,
            attempts = $6,
            locked_at = $7,
            locked_by = $8,
            last_error = $9,
            last_run_at = $10,
            completed_at = $11,
            updated_at = $12
        WHERE id = $1::uuid
        "#,
    )
    .bind(&job.id)
    .bind(job.run_at)
    .bind(job.remaining_runs)
    .bind(job.run_count)
    .bind(&job.status)
    .bind(job.attempts)
    .bind(job.locked_at)
    .bind(&job.locked_by)
    .bind(&job.last_error)
    .bind(job.last_run_at)
    .bind(job.completed_at)
    .bind(job.updated_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

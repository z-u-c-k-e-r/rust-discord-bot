mod memory;
mod model;
mod postgres;
pub mod time;

use std::{sync::Arc, time::Duration as StdDuration};

use anyhow::{Context as _, Result, anyhow};
use chrono::{Duration, Utc};
use serenity::{
    all::{ChannelId, UserId},
    builder::{CreateAllowedMentions, CreateMessage},
    http::Http,
};
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use crate::state::AppState;

pub use memory::MemorySchedulerStore;
pub use model::{
    CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, STATUS_ACTIVE,
    STATUS_CANCELLED, STATUS_COMPLETED, STATUS_FAILED, STATUS_PAUSED, STATUS_PROCESSING,
    ScheduledJob,
};
pub use postgres::PostgresSchedulerStore;

const POLL_INTERVAL_SECONDS: u64 = 5;
const LEASE_SECONDS: i64 = 60;
const CLAIM_BATCH_SIZE: u8 = 25;

#[derive(Clone)]
pub enum SchedulerStore {
    Memory(MemorySchedulerStore),
    Postgres(PostgresSchedulerStore),
}

impl SchedulerStore {
    pub async fn connect(database_url: Option<&str>) -> Result<Self> {
        match database_url {
            Some(url) => Ok(Self::Postgres(PostgresSchedulerStore::connect(url).await?)),
            None => {
                tracing::warn!(
                    "DATABASE_URL is not set; scheduled jobs are volatile and disappear on restart"
                );
                Ok(Self::Memory(MemorySchedulerStore::default()))
            }
        }
    }

    pub const fn memory() -> Self {
        Self::Memory(MemorySchedulerStore::default())
    }

    pub async fn create_job(
        &self,
        job: NewScheduledJob,
        max_jobs: u16,
    ) -> Result<CreateJobOutcome> {
        match self {
            Self::Memory(store) => Ok(store.create_job(job, max_jobs)),
            Self::Postgres(store) => Ok(store.create_job(job, max_jobs).await?),
        }
    }

    pub async fn list_jobs(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> Result<Vec<ScheduledJob>> {
        match self {
            Self::Memory(store) => {
                Ok(store.list_jobs(guild_id, creator_user_id, include_all, limit))
            }
            Self::Postgres(store) => Ok(store
                .list_jobs(guild_id, creator_user_id, include_all, limit)
                .await?),
        }
    }

    pub async fn mutate_job(
        &self,
        guild_id: &str,
        job_id: &str,
        actor_user_id: &str,
        allow_any: bool,
        mutation: JobMutation,
    ) -> Result<JobMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.mutate_job(
                guild_id,
                job_id,
                actor_user_id,
                allow_any,
                mutation,
            )),
            Self::Postgres(store) => Ok(store
                .mutate_job(guild_id, job_id, actor_user_id, allow_any, mutation)
                .await?),
        }
    }

    pub async fn claim_due(
        &self,
        worker_id: &str,
        now: chrono::DateTime<Utc>,
        stale_before: chrono::DateTime<Utc>,
        limit: u8,
    ) -> Result<Vec<ScheduledJob>> {
        match self {
            Self::Memory(store) => Ok(store.claim_due(worker_id, now, stale_before, limit)),
            Self::Postgres(store) => Ok(store
                .claim_due(worker_id, now, stale_before, limit)
                .await?),
        }
    }

    pub async fn mark_succeeded(
        &self,
        job_id: &str,
        worker_id: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<Option<ScheduledJob>> {
        match self {
            Self::Memory(store) => Ok(store.mark_succeeded(job_id, worker_id, now)),
            Self::Postgres(store) => {
                Ok(store.mark_succeeded(job_id, worker_id, now).await?)
            }
        }
    }

    pub async fn mark_failed(
        &self,
        job_id: &str,
        worker_id: &str,
        now: chrono::DateTime<Utc>,
        error: &str,
    ) -> Result<Option<ScheduledJob>> {
        match self {
            Self::Memory(store) => Ok(store.mark_failed(job_id, worker_id, now, error)),
            Self::Postgres(store) => Ok(store.mark_failed(job_id, worker_id, now, error).await?),
        }
    }
}

pub async fn run_worker(http: Arc<Http>, state: AppState) -> Result<()> {
    let worker_id = Uuid::new_v4().to_string();
    let mut interval = tokio::time::interval(StdDuration::from_secs(POLL_INTERVAL_SECONDS));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    tracing::info!(%worker_id, "persistent scheduler worker started");
    loop {
        interval.tick().await;
        if let Err(error) = process_due_jobs(&http, &state, &worker_id).await {
            tracing::error!(%worker_id, ?error, "scheduler polling iteration failed");
        }
    }
}

async fn process_due_jobs(http: &Http, state: &AppState, worker_id: &str) -> Result<()> {
    let now = Utc::now();
    let jobs = state
        .scheduler
        .claim_due(
            worker_id,
            now,
            now - Duration::seconds(LEASE_SECONDS),
            CLAIM_BATCH_SIZE,
        )
        .await?;

    for job in jobs {
        match deliver_job(http, &job).await {
            Ok(()) => {
                let Some(updated) = state
                    .scheduler
                    .mark_succeeded(&job.id, worker_id, Utc::now())
                    .await?
                else {
                    tracing::warn!(job_id = %job.id, "scheduler job lost its lease before completion");
                    continue;
                };

                record_delivery_audit(state, &updated, "scheduled_job_delivered", None).await;
            }
            Err(error) => {
                let error_message = format!("{error:#}");
                let Some(updated) = state
                    .scheduler
                    .mark_failed(&job.id, worker_id, Utc::now(), &error_message)
                    .await?
                else {
                    tracing::warn!(job_id = %job.id, "failed scheduler job lost its lease");
                    continue;
                };

                let event = if updated.status == STATUS_FAILED {
                    "scheduled_job_failed"
                } else {
                    "scheduled_job_retry_scheduled"
                };
                record_delivery_audit(state, &updated, event, Some(&error_message)).await;
                tracing::warn!(
                    job_id = %updated.id,
                    status = %updated.status,
                    attempts = updated.attempts,
                    ?error,
                    "scheduled message delivery failed"
                );
            }
        }
    }

    Ok(())
}

async fn deliver_job(http: &Http, job: &ScheduledJob) -> Result<()> {
    let channel_id = ChannelId::new(parse_snowflake(&job.channel_id)?);
    let creator_id = UserId::new(parse_snowflake(&job.creator_user_id)?);
    let (content, allowed_mentions) = if job.mention_creator {
        (
            format!("<@{}> {}", creator_id.get(), job.content),
            CreateAllowedMentions::new().users([creator_id]),
        )
    } else {
        (job.content.clone(), CreateAllowedMentions::new())
    };

    channel_id
        .send_message(
            http,
            CreateMessage::new()
                .content(content)
                .allowed_mentions(allowed_mentions),
        )
        .await
        .with_context(|| format!("cannot deliver scheduled job {}", job.id))?;
    Ok(())
}

async fn record_delivery_audit(
    state: &AppState,
    job: &ScheduledJob,
    event: &str,
    error: Option<&str>,
) {
    if let Err(audit_error) = state
        .storage
        .record_audit(
            Some(&job.guild_id),
            Some(&job.creator_user_id),
            "scheduler",
            event,
            serde_json::json!({
                "job_id": job.id,
                "channel_id": job.channel_id,
                "status": job.status,
                "run_count": job.run_count,
                "attempts": job.attempts,
                "next_run_at": job.run_at,
                "error": error,
            }),
        )
        .await
    {
        tracing::error!(job_id = %job.id, ?audit_error, "cannot record scheduler audit event");
    }
}

fn parse_snowflake(value: &str) -> Result<u64> {
    let value = value
        .parse::<u64>()
        .with_context(|| format!("{value:?} is not a Discord snowflake"))?;
    if value == 0 {
        return Err(anyhow!("Discord snowflake cannot be zero"));
    }
    Ok(value)
}

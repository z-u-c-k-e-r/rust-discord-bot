use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use chrono::{DateTime, Utc};

use super::model::{
    CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, STATUS_ACTIVE,
    STATUS_CANCELLED, STATUS_PAUSED, STATUS_PROCESSING, ScheduledJob,
};

#[derive(Clone, Default)]
pub struct MemorySchedulerStore {
    jobs: Arc<Mutex<HashMap<String, ScheduledJob>>>,
}

impl MemorySchedulerStore {
    pub fn create_job(&self, job: NewScheduledJob, max_jobs: u16) -> CreateJobOutcome {
        let mut jobs = self.lock();
        let pending_count = jobs
            .values()
            .filter(|existing| {
                existing.guild_id == job.guild_id
                    && existing.creator_user_id == job.creator_user_id
                    && existing.is_pending()
            })
            .count();

        if pending_count >= usize::from(max_jobs) {
            return CreateJobOutcome::LimitReached { limit: max_jobs };
        }

        let job = job.materialize(Utc::now());
        jobs.insert(job.id.clone(), job.clone());
        CreateJobOutcome::Created(Box::new(job))
    }

    pub fn list_jobs(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> Vec<ScheduledJob> {
        let jobs = self.lock();
        let mut result = jobs
            .values()
            .filter(|job| {
                job.guild_id == guild_id
                    && job.is_pending()
                    && (include_all || job.creator_user_id == creator_user_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        result.sort_by(|left, right| {
            left.run_at
                .cmp(&right.run_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        result.truncate(usize::from(limit));
        result
    }

    pub fn mutate_job(
        &self,
        guild_id: &str,
        job_id: &str,
        actor_user_id: &str,
        allow_any: bool,
        mutation: JobMutation,
    ) -> JobMutationOutcome {
        let mut jobs = self.lock();
        let Some(job) = jobs.get_mut(job_id) else {
            return JobMutationOutcome::NotFound;
        };
        if job.guild_id != guild_id {
            return JobMutationOutcome::NotFound;
        }
        if !allow_any && job.creator_user_id != actor_user_id {
            return JobMutationOutcome::Forbidden;
        }

        let valid_state = match mutation {
            JobMutation::Cancel => job.is_pending(),
            JobMutation::Pause => job.status == STATUS_ACTIVE,
            JobMutation::Resume => job.status == STATUS_PAUSED,
        };
        if !valid_state {
            return JobMutationOutcome::InvalidState {
                current_status: job.status.clone(),
            };
        }

        let now = Utc::now();
        match mutation {
            JobMutation::Cancel => {
                job.status = STATUS_CANCELLED.to_owned();
                job.completed_at = Some(now);
            }
            JobMutation::Pause => {
                job.status = STATUS_PAUSED.to_owned();
            }
            JobMutation::Resume => {
                job.status = STATUS_ACTIVE.to_owned();
                if job.run_at <= now {
                    job.run_at = now + chrono::Duration::seconds(1);
                }
                job.completed_at = None;
            }
        }
        job.locked_at = None;
        job.locked_by = None;
        job.updated_at = now;
        JobMutationOutcome::Updated(Box::new(job.clone()))
    }

    pub fn claim_due(
        &self,
        worker_id: &str,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
        limit: u8,
    ) -> Vec<ScheduledJob> {
        let mut jobs = self.lock();
        let mut candidates = jobs
            .values()
            .filter(|job| job.is_claimable(now, stale_before))
            .map(|job| (job.id.clone(), job.run_at))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        candidates.truncate(usize::from(limit));

        let mut claimed = Vec::with_capacity(candidates.len());
        for (job_id, _) in candidates {
            if let Some(job) = jobs.get_mut(&job_id) {
                job.claim(worker_id, now);
                claimed.push(job.clone());
            }
        }
        claimed
    }

    pub fn mark_succeeded(
        &self,
        job_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
    ) -> Option<ScheduledJob> {
        let mut jobs = self.lock();
        let job = jobs.get_mut(job_id)?;
        if job.status != STATUS_PROCESSING || job.locked_by.as_deref() != Some(worker_id) {
            return None;
        }
        job.mark_succeeded(now);
        Some(job.clone())
    }

    pub fn mark_failed(
        &self,
        job_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        error: &str,
    ) -> Option<ScheduledJob> {
        let mut jobs = self.lock();
        let job = jobs.get_mut(job_id)?;
        if job.status != STATUS_PROCESSING || job.locked_by.as_deref() != Some(worker_id) {
            return None;
        }
        job.mark_failed(now, error);
        Some(job.clone())
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, ScheduledJob>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

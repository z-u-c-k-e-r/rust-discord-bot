use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_PAUSED: &str = "paused";
pub const STATUS_PROCESSING: &str = "processing";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_FAILED: &str = "failed";

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduledJob {
    pub id: String,
    pub guild_id: String,
    pub module_id: String,
    pub channel_id: String,
    pub creator_user_id: String,
    pub content: String,
    pub mention_creator: bool,
    pub run_at: DateTime<Utc>,
    pub repeat_every_seconds: Option<i64>,
    pub remaining_runs: Option<i64>,
    pub run_count: i64,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub last_error: Option<String>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ScheduledJob {
    pub fn is_pending(&self) -> bool {
        matches!(
            self.status.as_str(),
            STATUS_ACTIVE | STATUS_PAUSED | STATUS_PROCESSING
        )
    }

    pub fn is_claimable(&self, now: DateTime<Utc>, stale_before: DateTime<Utc>) -> bool {
        (self.status == STATUS_ACTIVE && self.run_at <= now)
            || (self.status == STATUS_PROCESSING
                && self
                    .locked_at
                    .is_some_and(|locked_at| locked_at < stale_before))
    }

    pub fn claim(&mut self, worker_id: &str, now: DateTime<Utc>) {
        self.status = STATUS_PROCESSING.to_owned();
        self.locked_by = Some(worker_id.to_owned());
        self.locked_at = Some(now);
        self.attempts = self.attempts.saturating_add(1);
        self.updated_at = now;
    }

    pub fn mark_succeeded(&mut self, now: DateTime<Utc>) {
        self.run_count = self.run_count.saturating_add(1);
        self.last_run_at = Some(now);
        self.last_error = None;
        self.attempts = 0;
        self.locked_at = None;
        self.locked_by = None;
        self.updated_at = now;

        let Some(repeat_every_seconds) = self.repeat_every_seconds else {
            self.status = STATUS_COMPLETED.to_owned();
            self.remaining_runs = Some(0);
            self.completed_at = Some(now);
            return;
        };

        let should_repeat = self.remaining_runs.is_none_or(|remaining| remaining > 1);
        if !should_repeat {
            self.status = STATUS_COMPLETED.to_owned();
            self.remaining_runs = Some(0);
            self.completed_at = Some(now);
            return;
        }

        if let Some(remaining_runs) = self.remaining_runs.as_mut() {
            *remaining_runs = remaining_runs.saturating_sub(1);
        }
        let scheduled_next = self.run_at + Duration::seconds(repeat_every_seconds.max(1));
        self.run_at = scheduled_next.max(now + Duration::seconds(1));
        self.status = STATUS_ACTIVE.to_owned();
        self.completed_at = None;
    }

    pub fn mark_failed(&mut self, now: DateTime<Utc>, error: &str) {
        self.last_error = Some(truncate(error, 1_000));
        self.locked_at = None;
        self.locked_by = None;
        self.updated_at = now;

        if self.attempts >= self.max_attempts {
            self.status = STATUS_FAILED.to_owned();
            self.completed_at = Some(now);
            return;
        }

        self.status = STATUS_ACTIVE.to_owned();
        self.run_at = now + Duration::seconds(i64::from(self.retry_delay_seconds()));
    }

    pub fn retry_delay_seconds(&self) -> u32 {
        let exponent = self.attempts.saturating_sub(1).clamp(0, 6) as u32;
        5_u32
            .saturating_mul(2_u32.saturating_pow(exponent))
            .min(300)
    }

    pub fn short_id(&self) -> &str {
        self.id.get(..8).unwrap_or(&self.id)
    }
}

#[derive(Clone, Debug)]
pub struct NewScheduledJob {
    pub guild_id: String,
    pub module_id: String,
    pub channel_id: String,
    pub creator_user_id: String,
    pub content: String,
    pub mention_creator: bool,
    pub run_at: DateTime<Utc>,
    pub repeat_every_seconds: Option<i64>,
    pub remaining_runs: Option<i64>,
    pub max_attempts: i32,
}

impl NewScheduledJob {
    pub fn materialize(self, now: DateTime<Utc>) -> ScheduledJob {
        ScheduledJob {
            id: Uuid::new_v4().to_string(),
            guild_id: self.guild_id,
            module_id: self.module_id,
            channel_id: self.channel_id,
            creator_user_id: self.creator_user_id,
            content: self.content,
            mention_creator: self.mention_creator,
            run_at: self.run_at,
            repeat_every_seconds: self.repeat_every_seconds,
            remaining_runs: self.remaining_runs,
            run_count: 0,
            status: STATUS_ACTIVE.to_owned(),
            attempts: 0,
            max_attempts: self.max_attempts,
            locked_at: None,
            locked_by: None,
            last_error: None,
            last_run_at: None,
            completed_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CreateJobOutcome {
    Created(Box<ScheduledJob>),
    LimitReached { limit: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobMutation {
    Cancel,
    Pause,
    Resume,
}

impl JobMutation {
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Cancel => "scheduled_job_cancelled",
            Self::Pause => "scheduled_job_paused",
            Self::Resume => "scheduled_job_resumed",
        }
    }
}

#[derive(Clone, Debug)]
pub enum JobMutationOutcome {
    Updated(Box<ScheduledJob>),
    NotFound,
    Forbidden,
    InvalidState { current_status: String },
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{NewScheduledJob, STATUS_ACTIVE, STATUS_COMPLETED, STATUS_FAILED};

    fn recurring_job(remaining_runs: Option<i64>) -> super::ScheduledJob {
        let now = Utc::now();
        NewScheduledJob {
            guild_id: "1".to_owned(),
            module_id: "scheduler".to_owned(),
            channel_id: "2".to_owned(),
            creator_user_id: "3".to_owned(),
            content: "test".to_owned(),
            mention_creator: false,
            run_at: now,
            repeat_every_seconds: Some(60),
            remaining_runs,
            max_attempts: 5,
        }
        .materialize(now)
    }

    #[test]
    fn finite_recurrence_completes_after_the_last_run() {
        let now = Utc::now();
        let mut job = recurring_job(Some(2));
        job.mark_succeeded(now);
        assert_eq!(job.status, STATUS_ACTIVE);
        assert_eq!(job.remaining_runs, Some(1));

        job.mark_succeeded(now + Duration::minutes(1));
        assert_eq!(job.status, STATUS_COMPLETED);
        assert_eq!(job.remaining_runs, Some(0));
    }

    #[test]
    fn exhausted_retries_fail_the_job() {
        let now = Utc::now();
        let mut job = recurring_job(None);
        job.attempts = job.max_attempts;
        job.mark_failed(now, "network error");
        assert_eq!(job.status, STATUS_FAILED);
    }
}

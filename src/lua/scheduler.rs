use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_SCHEDULE_HORIZON_SECONDS: u64 = 315_576_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchedulerOperation {
    Create {
        when: String,
        content: String,
        #[serde(default)]
        channel_id: Option<String>,
        #[serde(default)]
        repeat: Option<String>,
        #[serde(default)]
        repeat_count: Option<u32>,
        #[serde(default)]
        mention_creator: bool,
        max_jobs: u16,
        minimum_delay_seconds: u32,
        maximum_delay_seconds: u64,
    },
    List {
        #[serde(default)]
        scope: SchedulerScope,
        #[serde(default = "default_list_limit")]
        limit: u8,
    },
    Cancel {
        job_id: String,
    },
    Pause {
        job_id: String,
    },
    Resume {
        job_id: String,
    },
}

impl SchedulerOperation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Create {
                when,
                content,
                channel_id,
                repeat,
                repeat_count,
                max_jobs,
                minimum_delay_seconds,
                maximum_delay_seconds,
                ..
            } => {
                let when_length = when.trim().chars().count();
                if when_length == 0 || when_length > 128 {
                    return Err("schedule time must contain 1 to 128 characters".to_owned());
                }

                let content_length = content.trim().chars().count();
                if content_length == 0 || content_length > 1_800 {
                    return Err("scheduled content must contain 1 to 1800 characters".to_owned());
                }

                if let Some(channel_id) = channel_id {
                    validate_snowflake(channel_id)?;
                }

                if let Some(repeat) = repeat {
                    let repeat_length = repeat.trim().chars().count();
                    if repeat_length == 0 || repeat_length > 64 {
                        return Err(
                            "repeat interval must contain 1 to 64 characters".to_owned()
                        );
                    }
                }

                if let Some(repeat_count) = repeat_count {
                    if repeat.is_none() {
                        return Err("repeat_count requires a repeat interval".to_owned());
                    }
                    if !(2..=10_000).contains(repeat_count) {
                        return Err("repeat_count must be between 2 and 10000".to_owned());
                    }
                }

                if !(1..=500).contains(max_jobs) {
                    return Err("max_jobs must be between 1 and 500".to_owned());
                }
                if !(10..=3_600).contains(minimum_delay_seconds) {
                    return Err(
                        "minimum_delay_seconds must be between 10 and 3600".to_owned()
                    );
                }
                if *maximum_delay_seconds < u64::from(*minimum_delay_seconds)
                    || *maximum_delay_seconds > MAX_SCHEDULE_HORIZON_SECONDS
                {
                    return Err(
                        "maximum_delay_seconds must be at least the minimum and no more than ten years"
                            .to_owned(),
                    );
                }

                Ok(())
            }
            Self::List { limit, .. } => {
                if !(1..=20).contains(limit) {
                    return Err("scheduler list limit must be between 1 and 20".to_owned());
                }
                Ok(())
            }
            Self::Cancel { job_id } | Self::Pause { job_id } | Self::Resume { job_id } => {
                validate_job_id(job_id)
            }
        }
    }

    pub const fn requires_manage_guild(&self) -> bool {
        match self {
            Self::Create {
                channel_id,
                repeat,
                repeat_count,
                ..
            } => channel_id.is_some() || repeat.is_some() || repeat_count.is_some(),
            Self::List { scope, .. } => matches!(scope, SchedulerScope::All),
            Self::Cancel { .. } | Self::Pause { .. } | Self::Resume { .. } => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerScope {
    #[default]
    Mine,
    All,
}

fn validate_job_id(value: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "scheduler job id must be a valid UUID".to_owned())
}

fn validate_snowflake(value: &str) -> Result<(), String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{value:?} is not a Discord snowflake"))?;
    if parsed == 0 {
        return Err("Discord snowflakes must be greater than zero".to_owned());
    }
    Ok(())
}

const fn default_list_limit() -> u8 {
    10
}

#[cfg(test)]
mod tests {
    use super::{SchedulerOperation, SchedulerScope};

    #[test]
    fn recurring_jobs_are_staff_operations() {
        let operation = SchedulerOperation::Create {
            when: "15m".to_owned(),
            content: "Maintenance".to_owned(),
            channel_id: None,
            repeat: Some("1h".to_owned()),
            repeat_count: None,
            mention_creator: false,
            max_jobs: 10,
            minimum_delay_seconds: 10,
            maximum_delay_seconds: 86_400,
        };

        assert!(operation.validate().is_ok());
        assert!(operation.requires_manage_guild());
    }

    #[test]
    fn listing_own_jobs_does_not_require_staff_permissions() {
        let operation = SchedulerOperation::List {
            scope: SchedulerScope::Mine,
            limit: 10,
        };

        assert!(operation.validate().is_ok());
        assert!(!operation.requires_manage_guild());
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModerationCaseOperation {
    Create {
        subject_user_id: String,
        #[serde(default = "default_case_kind")]
        kind: String,
        reason: String,
        #[serde(default = "default_severity")]
        severity: String,
        #[serde(default)]
        points: i32,
        #[serde(default = "default_true")]
        visible_to_subject: bool,
        #[serde(default)]
        duration_seconds: Option<u64>,
    },
    View {
        case_number: i64,
    },
    List {
        #[serde(default)]
        subject_user_id: Option<String>,
        #[serde(default = "default_case_list_limit")]
        limit: u8,
        #[serde(default)]
        visible_to_subject_only: bool,
    },
    AddNote {
        case_number: i64,
        body: String,
        #[serde(default)]
        visible_to_subject: bool,
    },
    AddEvidence {
        case_number: i64,
        label: String,
        value: String,
    },
    Void {
        case_number: i64,
        #[serde(default)]
        expected_version: i64,
        reason: String,
    },
    Restore {
        case_number: i64,
        #[serde(default)]
        expected_version: i64,
    },
    Stats {
        #[serde(default)]
        moderator_user_id: Option<String>,
    },
}

impl ModerationCaseOperation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Create {
                subject_user_id,
                kind,
                reason,
                severity,
                points,
                duration_seconds,
                ..
            } => {
                validate_snowflake(subject_user_id)?;
                validate_kind(kind)?;
                validate_severity(severity)?;
                validate_text(reason, 1, 2000, "case reason")?;
                if !(0..=10_000).contains(points) {
                    return Err("case points must be between 0 and 10000".to_owned());
                }
                if let Some(duration) = duration_seconds
                    && (*duration == 0 || *duration > 3653 * 24 * 3600)
                {
                    return Err("case duration exceeds allowed limit".to_owned());
                }
                Ok(())
            }
            Self::View { case_number } => {
                if *case_number < 1 {
                    return Err("case number must be greater than zero".to_owned());
                }
                Ok(())
            }
            Self::List {
                subject_user_id,
                limit,
                ..
            } => {
                if let Some(user_id) = subject_user_id {
                    validate_snowflake(user_id)?;
                }
                if !(1..=25).contains(limit) {
                    return Err("case list limit must be between 1 and 25".to_owned());
                }
                Ok(())
            }
            Self::AddNote {
                case_number, body, ..
            } => {
                if *case_number < 1 {
                    return Err("case number must be greater than zero".to_owned());
                }
                validate_text(body, 1, 2000, "case note body")?;
                Ok(())
            }
            Self::AddEvidence {
                case_number,
                label,
                value,
            } => {
                if *case_number < 1 {
                    return Err("case number must be greater than zero".to_owned());
                }
                validate_text(label, 1, 200, "evidence label")?;
                validate_text(value, 1, 4096, "evidence value")?;
                Ok(())
            }
            Self::Void {
                case_number,
                reason,
                ..
            } => {
                if *case_number < 1 {
                    return Err("case number must be greater than zero".to_owned());
                }
                validate_text(reason, 1, 2000, "void reason")?;
                Ok(())
            }
            Self::Restore { case_number, .. } => {
                if *case_number < 1 {
                    return Err("case number must be greater than zero".to_owned());
                }
                Ok(())
            }
            Self::Stats { moderator_user_id } => {
                if let Some(user_id) = moderator_user_id {
                    validate_snowflake(user_id)?;
                }
                Ok(())
            }
        }
    }
}

fn validate_kind(kind: &str) -> Result<(), String> {
    match kind {
        "warning" | "staff_note" | "timeout" | "kick" | "ban" | "unban" | "automod" | "other" => {
            Ok(())
        }
        _ => Err(format!("unknown moderation case kind: {kind}")),
    }
}

fn validate_severity(severity: &str) -> Result<(), String> {
    match severity {
        "info" | "low" | "medium" | "high" | "critical" => Ok(()),
        _ => Err(format!("unknown moderation case severity: {severity}")),
    }
}

fn validate_text(value: &str, minimum: usize, maximum: usize, label: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.chars().count() < minimum {
        return Err(format!("{label} cannot be empty"));
    }
    if trimmed.chars().count() > maximum {
        return Err(format!("{label} exceeds {maximum} characters"));
    }
    Ok(())
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

fn default_case_kind() -> String {
    "warning".to_owned()
}

fn default_severity() -> String {
    "medium".to_owned()
}

const fn default_case_list_limit() -> u8 {
    10
}

const fn default_true() -> bool {
    true
}

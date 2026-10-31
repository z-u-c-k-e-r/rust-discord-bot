use std::{fmt, str::FromStr};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_REASON_LENGTH: usize = 2_000;
pub const MAX_NOTE_LENGTH: usize = 2_000;
pub const MAX_EVIDENCE_LABEL_LENGTH: usize = 200;
pub const MAX_EVIDENCE_VALUE_LENGTH: usize = 4_096;
pub const MAX_SOURCE_MODULE_LENGTH: usize = 64;
pub const MAX_POINTS: i32 = 10_000;
pub const MAX_LIST_LIMIT: u16 = 100;
pub const MAX_EXPIRY_DAYS: i64 = 3_653;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    Warning,
    StaffNote,
    Timeout,
    Kick,
    Ban,
    Unban,
    Softban,
    Automod,
    Other,
}

impl CaseKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::StaffNote => "staff_note",
            Self::Timeout => "timeout",
            Self::Kick => "kick",
            Self::Ban => "ban",
            Self::Unban => "unban",
            Self::Softban => "softban",
            Self::Automod => "automod",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for CaseKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CaseKind {
    type Err = CaseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "warning" | "warn" => Ok(Self::Warning),
            "staff_note" | "note" => Ok(Self::StaffNote),
            "timeout" => Ok(Self::Timeout),
            "kick" => Ok(Self::Kick),
            "ban" => Ok(Self::Ban),
            "unban" => Ok(Self::Unban),
            "softban" => Ok(Self::Softban),
            "automod" => Ok(Self::Automod),
            "other" => Ok(Self::Other),
            _ => Err(CaseError::Validation(format!(
                "unsupported moderation case kind: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Active,
    Expired,
    Voided,
}

impl CaseStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Voided => "voided",
        }
    }
}

impl fmt::Display for CaseStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CaseStatus {
    type Err = CaseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "voided" => Ok(Self::Voided),
            _ => Err(CaseError::Validation(format!(
                "unsupported moderation case status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CaseSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl CaseSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for CaseSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CaseSeverity {
    type Err = CaseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "info" => Ok(Self::Info),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(CaseError::Validation(format!(
                "unsupported moderation case severity: {value}"
            ))),
        }
    }
}

mod snowflake_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrInt {
            String(String),
            Int(u64),
        }

        match StringOrInt::deserialize(deserializer)? {
            StringOrInt::String(s) => s.parse::<u64>().map_err(serde::de::Error::custom),
            StringOrInt::Int(i) => Ok(i),
        }
    }
}

mod opt_snowflake_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_some(&v.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum OptStringOrInt {
            String(String),
            Int(u64),
        }

        match Option::<OptStringOrInt>::deserialize(deserializer)? {
            Some(OptStringOrInt::String(s)) => {
                if s.trim().is_empty() {
                    Ok(None)
                } else {
                    s.parse::<u64>().map(Some).map_err(serde::de::Error::custom)
                }
            }
            Some(OptStringOrInt::Int(i)) => Ok(Some(i)),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModerationCase {
    pub id: Uuid,
    pub case_number: i64,
    #[serde(with = "snowflake_string")]
    pub guild_id: u64,
    #[serde(with = "snowflake_string")]
    pub subject_user_id: u64,
    #[serde(with = "snowflake_string")]
    pub actor_user_id: u64,
    pub kind: CaseKind,
    pub status: CaseStatus,
    pub severity: CaseSeverity,
    pub points: i32,
    pub reason: String,
    pub source_module: String,
    pub visible_to_subject: bool,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(with = "opt_snowflake_string", default)]
    pub voided_by_user_id: Option<u64>,
    pub void_reason: Option<String>,
    pub voided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseNote {
    pub id: Uuid,
    pub case_id: Uuid,
    #[serde(with = "snowflake_string")]
    pub author_user_id: u64,
    pub body: String,
    pub visible_to_subject: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseEvidence {
    pub id: Uuid,
    pub case_id: Uuid,
    #[serde(with = "snowflake_string")]
    pub author_user_id: u64,
    pub label: String,
    pub value: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseEvent {
    pub id: Uuid,
    pub case_id: Uuid,
    #[serde(with = "snowflake_string")]
    pub actor_user_id: u64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseDetails {
    pub case: ModerationCase,
    pub notes: Vec<CaseNote>,
    pub evidence: Vec<CaseEvidence>,
    pub events: Vec<CaseEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModeratorStats {
    pub guild_id: u64,
    pub actor_user_id: Option<u64>,
    pub total_cases: u64,
    pub active_cases: u64,
    pub expired_cases: u64,
    pub voided_cases: u64,
    pub warning_cases: u64,
    pub timeout_cases: u64,
    pub kick_cases: u64,
    pub ban_cases: u64,
    pub total_points: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateCase {
    pub guild_id: u64,
    pub subject_user_id: u64,
    pub actor_user_id: u64,
    pub kind: CaseKind,
    pub severity: CaseSeverity,
    pub points: i32,
    pub reason: String,
    pub source_module: String,
    pub visible_to_subject: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CreateCase {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), CaseError> {
        validate_snowflake(self.guild_id, "guild_id")?;
        validate_snowflake(self.subject_user_id, "subject_user_id")?;
        validate_snowflake(self.actor_user_id, "actor_user_id")?;
        validate_text(&self.reason, "reason", 1, MAX_REASON_LENGTH)?;
        validate_text(
            &self.source_module,
            "source_module",
            1,
            MAX_SOURCE_MODULE_LENGTH,
        )?;
        validate_points(self.points)?;
        validate_expiry(self.expires_at, now)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateCase {
    pub reason: Option<String>,
    pub severity: Option<CaseSeverity>,
    pub points: Option<i32>,
    pub visible_to_subject: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
    pub clear_expiry: bool,
}

impl UpdateCase {
    pub fn validate(&self, now: DateTime<Utc>) -> Result<(), CaseError> {
        if let Some(reason) = &self.reason {
            validate_text(reason, "reason", 1, MAX_REASON_LENGTH)?;
        }
        if let Some(points) = self.points {
            validate_points(points)?;
        }
        if self.clear_expiry && self.expires_at.is_some() {
            return Err(CaseError::Validation(
                "expires_at and clear_expiry cannot be used together".to_owned(),
            ));
        }
        validate_expiry(self.expires_at, now)?;
        Ok(())
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.reason.is_none()
            && self.severity.is_none()
            && self.points.is_none()
            && self.visible_to_subject.is_none()
            && self.expires_at.is_none()
            && !self.clear_expiry
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaseFilter {
    pub subject_user_id: Option<u64>,
    pub actor_user_id: Option<u64>,
    pub kind: Option<CaseKind>,
    pub status: Option<CaseStatus>,
    pub visible_to_subject_only: bool,
    pub limit: u16,
}

impl CaseFilter {
    pub fn validate(&self) -> Result<(), CaseError> {
        if let Some(value) = self.subject_user_id {
            validate_snowflake(value, "subject_user_id")?;
        }
        if let Some(value) = self.actor_user_id {
            validate_snowflake(value, "actor_user_id")?;
        }
        if !(1..=MAX_LIST_LIMIT).contains(&self.limit) {
            return Err(CaseError::Validation(format!(
                "limit must be between 1 and {MAX_LIST_LIMIT}"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CaseError {
    #[error("moderation case not found")]
    NotFound,
    #[error("moderation case version conflict: expected {expected}, actual {actual}")]
    VersionConflict { expected: i64, actual: i64 },
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

pub fn validate_note(body: &str) -> Result<(), CaseError> {
    validate_text(body, "note", 1, MAX_NOTE_LENGTH)
}

pub fn validate_evidence(label: &str, value: &str) -> Result<(), CaseError> {
    validate_text(label, "evidence label", 1, MAX_EVIDENCE_LABEL_LENGTH)?;
    validate_text(value, "evidence value", 1, MAX_EVIDENCE_VALUE_LENGTH)
}

pub fn validate_void_reason(reason: &str) -> Result<(), CaseError> {
    validate_text(reason, "void reason", 1, MAX_REASON_LENGTH)
}

fn validate_snowflake(value: u64, field: &str) -> Result<(), CaseError> {
    if value == 0 {
        return Err(CaseError::Validation(format!(
            "{field} must be a non-zero Discord snowflake"
        )));
    }
    Ok(())
}

fn validate_points(points: i32) -> Result<(), CaseError> {
    if !(0..=MAX_POINTS).contains(&points) {
        return Err(CaseError::Validation(format!(
            "points must be between 0 and {MAX_POINTS}"
        )));
    }
    Ok(())
}

fn validate_expiry(expires_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Result<(), CaseError> {
    if let Some(expires_at) = expires_at {
        if expires_at <= now {
            return Err(CaseError::Validation(
                "expires_at must be in the future".to_owned(),
            ));
        }
        if expires_at > now + Duration::days(MAX_EXPIRY_DAYS) {
            return Err(CaseError::Validation(format!(
                "expires_at cannot be more than {MAX_EXPIRY_DAYS} days in the future"
            )));
        }
    }
    Ok(())
}

fn validate_text(
    value: &str,
    field: &str,
    min_length: usize,
    max_length: usize,
) -> Result<(), CaseError> {
    let length = value.chars().count();
    if length < min_length || length > max_length {
        return Err(CaseError::Validation(format!(
            "{field} length must be between {min_length} and {max_length} characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn snowflakes_serialize_as_strings_and_deserialize_from_both() {
        let case = ModerationCase {
            id: Uuid::new_v4(),
            case_number: 1,
            guild_id: 1234567890123456789,
            subject_user_id: 9876543210987654321,
            actor_user_id: 1111222233334444555,
            kind: CaseKind::Warning,
            status: CaseStatus::Active,
            severity: CaseSeverity::Medium,
            points: 5,
            reason: "Testing snowflake serialization".to_string(),
            source_module: "moderation".to_string(),
            visible_to_subject: true,
            expires_at: None,
            voided_by_user_id: Some(5555666677778888999),
            void_reason: None,
            voided_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
        };

        let serialized = serde_json::to_string(&case).expect("case should serialize");
        assert!(serialized.contains(r#""guild_id":"1234567890123456789""#));
        assert!(serialized.contains(r#""subject_user_id":"9876543210987654321""#));
        assert!(serialized.contains(r#""actor_user_id":"1111222233334444555""#));
        assert!(serialized.contains(r#""voided_by_user_id":"5555666677778888999""#));

        let note = CaseNote {
            id: Uuid::new_v4(),
            case_id: case.id,
            author_user_id: 9876543210987654321,
            body: "Internal note".to_string(),
            visible_to_subject: false,
            created_at: Utc::now(),
        };
        let note_json = serde_json::to_string(&note).expect("note should serialize");
        assert!(note_json.contains(r#""author_user_id":"9876543210987654321""#));

        // Deserialization from JSON string
        let deserialized: ModerationCase =
            serde_json::from_str(&serialized).expect("should deserialize from string snowflakes");
        assert_eq!(deserialized.subject_user_id, 9876543210987654321);

        // Deserialization from legacy numeric snowflakes
        let legacy_json = serde_json::json!({
            "id": case.id,
            "case_number": 1,
            "guild_id": 1234567890123456789_u64,
            "subject_user_id": 9876543210987654321_u64,
            "actor_user_id": 1111222233334444555_u64,
            "kind": "warning",
            "status": "active",
            "severity": "medium",
            "points": 5,
            "reason": "Test",
            "source_module": "moderation",
            "visible_to_subject": true,
            "created_at": Utc::now(),
            "updated_at": Utc::now(),
            "version": 1
        });
        let from_numbers: ModerationCase = serde_json::from_value(legacy_json)
            .expect("should deserialize from numeric snowflakes");
        assert_eq!(from_numbers.subject_user_id, 9876543210987654321);
    }
}

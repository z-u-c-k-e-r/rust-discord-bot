use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TicketOperation {
    Open {
        subject: String,
        description: String,
        #[serde(default = "default_queue")]
        queue: String,
        policy: Box<TicketOpenPolicy>,
    },
    List {
        #[serde(default)]
        scope: TicketScope,
        #[serde(default = "default_list_limit")]
        limit: u8,
        #[serde(default)]
        support_role_ids: Vec<String>,
    },
    Info,
    Claim {
        #[serde(default)]
        support_role_ids: Vec<String>,
    },
    Unclaim {
        #[serde(default)]
        support_role_ids: Vec<String>,
    },
    Close {
        #[serde(default)]
        reason: Option<String>,
        policy: Box<TicketClosePolicy>,
    },
    Reopen {
        open_category_id: String,
        #[serde(default)]
        support_role_ids: Vec<String>,
    },
    AddMember {
        user_id: String,
        #[serde(default)]
        support_role_ids: Vec<String>,
        #[serde(default)]
        creator_can_manage_participants: bool,
    },
    RemoveMember {
        user_id: String,
        #[serde(default)]
        support_role_ids: Vec<String>,
        #[serde(default)]
        creator_can_manage_participants: bool,
    },
    Rename {
        name: String,
        #[serde(default)]
        support_role_ids: Vec<String>,
        #[serde(default)]
        creator_can_rename: bool,
    },
    SetPriority {
        priority: TicketPriority,
        #[serde(default)]
        support_role_ids: Vec<String>,
    },
    Transcript {
        #[serde(default)]
        support_role_ids: Vec<String>,
        #[serde(default)]
        log_channel_id: Option<String>,
        #[serde(default = "default_transcript_limit")]
        max_messages: u16,
    },
}

impl TicketOperation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Open {
                subject,
                description,
                queue,
                policy,
            } => {
                validate_text(subject, 1, 100, "ticket subject")?;
                validate_text(description, 1, 1_800, "ticket description")?;
                validate_identifier(queue, 1, 32, "ticket queue")?;
                policy.validate()
            }
            Self::List {
                limit,
                support_role_ids,
                ..
            } => {
                if !(1..=25).contains(limit) {
                    return Err("ticket list limit must be between 1 and 25".to_owned());
                }
                validate_role_ids(support_role_ids)
            }
            Self::Info => Ok(()),
            Self::Claim { support_role_ids }
            | Self::Unclaim { support_role_ids }
            | Self::SetPriority {
                support_role_ids, ..
            } => validate_role_ids(support_role_ids),
            Self::Close { reason, policy } => {
                validate_optional_text(reason, 512, "ticket close reason")?;
                policy.validate()
            }
            Self::Reopen {
                open_category_id,
                support_role_ids,
            } => {
                validate_snowflake(open_category_id)?;
                validate_role_ids(support_role_ids)
            }
            Self::AddMember {
                user_id,
                support_role_ids,
                ..
            }
            | Self::RemoveMember {
                user_id,
                support_role_ids,
                ..
            } => {
                validate_snowflake(user_id)?;
                validate_role_ids(support_role_ids)
            }
            Self::Rename {
                name,
                support_role_ids,
                ..
            } => {
                validate_text(name, 2, 100, "ticket channel name")?;
                validate_role_ids(support_role_ids)
            }
            Self::Transcript {
                support_role_ids,
                log_channel_id,
                max_messages,
            } => {
                validate_role_ids(support_role_ids)?;
                if let Some(channel_id) = log_channel_id {
                    validate_snowflake(channel_id)?;
                }
                if !(1..=1_000).contains(max_messages) {
                    return Err(
                        "ticket transcript max_messages must be between 1 and 1000".to_owned()
                    );
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketOpenPolicy {
    pub open_category_id: String,
    #[serde(default)]
    pub archive_category_id: Option<String>,
    #[serde(default)]
    pub support_role_ids: Vec<String>,
    #[serde(default)]
    pub log_channel_id: Option<String>,
    #[serde(default = "default_max_open")]
    pub max_open_per_user: u8,
    #[serde(default = "default_channel_prefix")]
    pub channel_name_prefix: String,
    #[serde(default)]
    pub welcome_message: Option<String>,
}

impl TicketOpenPolicy {
    fn validate(&self) -> Result<(), String> {
        validate_snowflake(&self.open_category_id)?;
        if let Some(category_id) = &self.archive_category_id {
            validate_snowflake(category_id)?;
        }
        if let Some(channel_id) = &self.log_channel_id {
            validate_snowflake(channel_id)?;
        }
        validate_role_ids(&self.support_role_ids)?;
        if !(1..=25).contains(&self.max_open_per_user) {
            return Err("max_open_per_user must be between 1 and 25".to_owned());
        }
        validate_identifier(
            &self.channel_name_prefix,
            1,
            24,
            "ticket channel prefix",
        )?;
        validate_optional_text(&self.welcome_message, 1_500, "ticket welcome message")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketClosePolicy {
    #[serde(default)]
    pub support_role_ids: Vec<String>,
    #[serde(default)]
    pub archive_category_id: Option<String>,
    #[serde(default)]
    pub log_channel_id: Option<String>,
    #[serde(default = "default_true")]
    pub creator_can_close: bool,
    #[serde(default = "default_true")]
    pub generate_transcript: bool,
    #[serde(default = "default_transcript_limit")]
    pub transcript_max_messages: u16,
}

impl TicketClosePolicy {
    fn validate(&self) -> Result<(), String> {
        validate_role_ids(&self.support_role_ids)?;
        if let Some(category_id) = &self.archive_category_id {
            validate_snowflake(category_id)?;
        }
        if let Some(channel_id) = &self.log_channel_id {
            validate_snowflake(channel_id)?;
        }
        if !(1..=1_000).contains(&self.transcript_max_messages) {
            return Err(
                "ticket transcript_max_messages must be between 1 and 1000".to_owned()
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketScope {
    #[default]
    Mine,
    All,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicketPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

impl TicketPriority {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

fn validate_role_ids(values: &[String]) -> Result<(), String> {
    if values.len() > 20 {
        return Err("a ticket policy cannot contain more than 20 support roles".to_owned());
    }

    let mut unique = HashSet::with_capacity(values.len());
    for value in values {
        validate_snowflake(value)?;
        if !unique.insert(value) {
            return Err("ticket support roles cannot contain duplicates".to_owned());
        }
    }
    Ok(())
}

fn validate_identifier(
    value: &str,
    minimum: usize,
    maximum: usize,
    label: &str,
) -> Result<(), String> {
    let value = value.trim();
    let length = value.chars().count();
    if length < minimum || length > maximum {
        return Err(format!(
            "{label} must contain between {minimum} and {maximum} characters"
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_')
    {
        return Err(format!(
            "{label} may contain only lowercase ASCII letters, digits, hyphens and underscores"
        ));
    }
    Ok(())
}

fn validate_text(
    value: &str,
    minimum: usize,
    maximum: usize,
    label: &str,
) -> Result<(), String> {
    let length = value.trim().chars().count();
    if length < minimum || length > maximum {
        return Err(format!(
            "{label} must contain between {minimum} and {maximum} characters"
        ));
    }
    Ok(())
}

fn validate_optional_text(
    value: &Option<String>,
    maximum: usize,
    label: &str,
) -> Result<(), String> {
    if let Some(value) = value {
        validate_text(value, 1, maximum, label)?;
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

fn default_queue() -> String {
    "support".to_owned()
}

const fn default_list_limit() -> u8 {
    10
}

const fn default_max_open() -> u8 {
    3
}

fn default_channel_prefix() -> String {
    "ticket".to_owned()
}

const fn default_transcript_limit() -> u16 {
    500
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{TicketOpenPolicy, TicketOperation, TicketPriority, TicketScope};

    #[test]
    fn validates_a_complete_open_request() {
        let operation = TicketOperation::Open {
            subject: "Problem z kontem".to_owned(),
            description: "Nie mogę zalogować się do panelu.".to_owned(),
            queue: "account-support".to_owned(),
            policy: Box::new(TicketOpenPolicy {
                open_category_id: "100000000000000001".to_owned(),
                archive_category_id: Some("100000000000000002".to_owned()),
                support_role_ids: vec!["100000000000000003".to_owned()],
                log_channel_id: Some("100000000000000004".to_owned()),
                max_open_per_user: 3,
                channel_name_prefix: "ticket".to_owned(),
                welcome_message: Some("Opisz dodatkowe szczegóły.".to_owned()),
            }),
        };
        assert!(operation.validate().is_ok());
    }

    #[test]
    fn rejects_duplicate_support_roles() {
        let operation = TicketOperation::SetPriority {
            priority: TicketPriority::Urgent,
            support_role_ids: vec![
                "100000000000000003".to_owned(),
                "100000000000000003".to_owned(),
            ],
        };
        assert!(operation.validate().is_err());
    }

    #[test]
    fn list_scope_defaults_to_the_caller() {
        let scope = TicketScope::default();
        assert_eq!(scope, TicketScope::Mine);
    }
}

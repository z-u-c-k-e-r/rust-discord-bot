use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const STATUS_PROVISIONING: &str = "provisioning";
pub const STATUS_OPEN: &str = "open";
pub const STATUS_CLAIMED: &str = "claimed";
pub const STATUS_CLOSED: &str = "closed";
pub const STATUS_FAILED: &str = "failed";

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketRecord {
    pub id: String,
    pub number: i64,
    pub guild_id: String,
    pub creator_user_id: String,
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub subject: String,
    pub description: String,
    pub queue: String,
    pub priority: String,
    pub status: String,
    pub claimed_by_user_id: Option<String>,
    pub first_response_at: Option<DateTime<Utc>>,
    pub last_activity_at: DateTime<Utc>,
    pub close_reason: Option<String>,
    pub closed_by_user_id: Option<String>,
    pub provisioning_error: Option<String>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl TicketRecord {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status.as_str(),
            STATUS_PROVISIONING | STATUS_OPEN | STATUS_CLAIMED
        )
    }

    pub fn is_open(&self) -> bool {
        matches!(self.status.as_str(), STATUS_OPEN | STATUS_CLAIMED)
    }

    pub fn short_id(&self) -> &str {
        self.id.get(..8).unwrap_or(&self.id)
    }

    pub fn display_number(&self) -> String {
        format!("{:06}", self.number.max(0))
    }

    pub fn activate(&mut self, channel_id: String, channel_name: String, now: DateTime<Utc>) {
        self.channel_id = Some(channel_id);
        self.channel_name = Some(channel_name);
        self.status = STATUS_OPEN.to_owned();
        self.provisioning_error = None;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
        self.last_activity_at = now;
    }

    pub fn fail_provisioning(&mut self, error: &str, now: DateTime<Utc>) {
        self.status = STATUS_FAILED.to_owned();
        self.provisioning_error = Some(truncate(error, 1_000));
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
        self.closed_at = Some(now);
    }

    pub fn claim(&mut self, actor_user_id: &str, now: DateTime<Utc>) {
        self.status = STATUS_CLAIMED.to_owned();
        self.claimed_by_user_id = Some(actor_user_id.to_owned());
        self.first_response_at.get_or_insert(now);
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }

    pub fn unclaim(&mut self, now: DateTime<Utc>) {
        self.status = STATUS_OPEN.to_owned();
        self.claimed_by_user_id = None;
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }

    pub fn close(&mut self, actor_user_id: &str, reason: Option<&str>, now: DateTime<Utc>) {
        self.status = STATUS_CLOSED.to_owned();
        self.close_reason = reason.map(|value| truncate(value, 512));
        self.closed_by_user_id = Some(actor_user_id.to_owned());
        self.closed_at = Some(now);
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }

    pub fn reopen(&mut self, now: DateTime<Utc>) {
        self.status = STATUS_OPEN.to_owned();
        self.claimed_by_user_id = None;
        self.close_reason = None;
        self.closed_by_user_id = None;
        self.closed_at = None;
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }

    pub fn rename(&mut self, channel_name: String, now: DateTime<Utc>) {
        self.channel_name = Some(channel_name);
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }

    pub fn set_priority(&mut self, priority: &str, now: DateTime<Utc>) {
        self.priority = priority.to_owned();
        self.last_activity_at = now;
        self.version = self.version.saturating_add(1);
        self.updated_at = now;
    }
}

#[derive(Clone, Debug)]
pub struct NewTicketReservation {
    pub guild_id: String,
    pub creator_user_id: String,
    pub subject: String,
    pub description: String,
    pub queue: String,
}

impl NewTicketReservation {
    pub fn materialize(self, number: i64, now: DateTime<Utc>) -> TicketRecord {
        TicketRecord {
            id: Uuid::new_v4().to_string(),
            number,
            guild_id: self.guild_id,
            creator_user_id: self.creator_user_id,
            channel_id: None,
            channel_name: None,
            subject: self.subject,
            description: self.description,
            queue: self.queue,
            priority: "normal".to_owned(),
            status: STATUS_PROVISIONING.to_owned(),
            claimed_by_user_id: None,
            first_response_at: None,
            last_activity_at: now,
            close_reason: None,
            closed_by_user_id: None,
            provisioning_error: None,
            version: 1,
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ReserveTicketOutcome {
    Reserved(Box<TicketRecord>),
    LimitReached { limit: u8, active_count: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketMutation {
    Claim,
    Unclaim,
    Close,
    Reopen,
    Rename,
    SetPriority,
}

impl TicketMutation {
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Claim => "ticket_claimed",
            Self::Unclaim => "ticket_unclaimed",
            Self::Close => "ticket_closed",
            Self::Reopen => "ticket_reopened",
            Self::Rename => "ticket_renamed",
            Self::SetPriority => "ticket_priority_changed",
        }
    }
}

#[derive(Clone, Debug)]
pub enum TicketMutationOutcome {
    Updated(Box<TicketRecord>),
    NotFound,
    Forbidden,
    InvalidState { current_status: String },
    Conflict { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketParticipant {
    pub ticket_id: String,
    pub user_id: String,
    pub added_by_user_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub enum ParticipantMutationOutcome {
    Added(TicketParticipant),
    Removed,
    NotFound,
    AlreadyPresent,
    CreatorProtected,
    InvalidState { current_status: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketTranscript {
    pub id: String,
    pub ticket_id: String,
    pub guild_id: String,
    pub channel_id: String,
    pub generated_by_user_id: String,
    pub message_count: i32,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketEvent {
    pub id: i64,
    pub ticket_id: String,
    pub guild_id: String,
    pub actor_user_id: Option<String>,
    pub event_type: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewTicketTranscript {
    pub ticket_id: String,
    pub guild_id: String,
    pub channel_id: String,
    pub generated_by_user_id: String,
    pub message_count: i32,
    pub content: String,
}

impl NewTicketTranscript {
    pub fn materialize(self, now: DateTime<Utc>) -> TicketTranscript {
        TicketTranscript {
            id: Uuid::new_v4().to_string(),
            ticket_id: self.ticket_id,
            guild_id: self.guild_id,
            channel_id: self.channel_id,
            generated_by_user_id: self.generated_by_user_id,
            message_count: self.message_count,
            content: self.content,
            created_at: now,
        }
    }
}

fn truncate(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        NewTicketReservation, STATUS_CLAIMED, STATUS_CLOSED, STATUS_OPEN, STATUS_PROVISIONING,
    };

    fn ticket() -> super::TicketRecord {
        NewTicketReservation {
            guild_id: "1".to_owned(),
            creator_user_id: "2".to_owned(),
            subject: "Problem".to_owned(),
            description: "Opis".to_owned(),
            queue: "support".to_owned(),
        }
        .materialize(42, Utc::now())
    }

    #[test]
    fn ticket_moves_through_the_expected_lifecycle() {
        let now = Utc::now();
        let mut ticket = ticket();
        assert_eq!(ticket.status, STATUS_PROVISIONING);

        ticket.activate("3".to_owned(), "ticket-000042".to_owned(), now);
        assert_eq!(ticket.status, STATUS_OPEN);

        ticket.claim("4", now);
        assert_eq!(ticket.status, STATUS_CLAIMED);
        assert_eq!(ticket.claimed_by_user_id.as_deref(), Some("4"));

        ticket.close("4", Some("Resolved"), now);
        assert_eq!(ticket.status, STATUS_CLOSED);
        assert_eq!(ticket.close_reason.as_deref(), Some("Resolved"));

        ticket.reopen(now);
        assert_eq!(ticket.status, STATUS_OPEN);
        assert!(ticket.closed_at.is_none());
    }
}

mod memory;
mod model;
mod postgres;

use anyhow::Result;

pub use memory::MemoryTicketStore;
pub use model::{
    NewTicketReservation, NewTicketTranscript, ParticipantMutationOutcome, ReserveTicketOutcome,
    STATUS_CLAIMED, STATUS_CLOSED, STATUS_FAILED, STATUS_OPEN, STATUS_PROVISIONING, TicketEvent,
    TicketMutation, TicketMutationOutcome, TicketParticipant, TicketRecord, TicketTranscript,
};
pub use postgres::PostgresTicketStore;

#[derive(Clone)]
pub enum TicketStore {
    Memory(MemoryTicketStore),
    Postgres(PostgresTicketStore),
}

impl TicketStore {
    pub async fn connect(database_url: Option<&str>) -> Result<Self> {
        match database_url {
            Some(url) => Ok(Self::Postgres(PostgresTicketStore::connect(url).await?)),
            None => {
                tracing::warn!(
                    "DATABASE_URL is not set; tickets and transcripts are volatile and disappear on restart"
                );
                Ok(Self::Memory(MemoryTicketStore::default()))
            }
        }
    }

    pub fn memory() -> Self {
        Self::Memory(MemoryTicketStore::default())
    }

    pub async fn reserve_ticket(
        &self,
        reservation: NewTicketReservation,
        max_open_per_user: u8,
    ) -> Result<ReserveTicketOutcome> {
        match self {
            Self::Memory(store) => {
                Ok(store.reserve_ticket(reservation, max_open_per_user))
            }
            Self::Postgres(store) => {
                Ok(store
                    .reserve_ticket(reservation, max_open_per_user)
                    .await?)
            }
        }
    }

    pub async fn activate_ticket(
        &self,
        ticket_id: &str,
        channel_id: &str,
        channel_name: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.activate_ticket(ticket_id, channel_id, channel_name)),
            Self::Postgres(store) => Ok(store
                .activate_ticket(ticket_id, channel_id, channel_name)
                .await?),
        }
    }

    pub async fn fail_provisioning(
        &self,
        ticket_id: &str,
        error: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.fail_provisioning(ticket_id, error)),
            Self::Postgres(store) => Ok(store.fail_provisioning(ticket_id, error).await?),
        }
    }

    pub async fn get_by_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> Result<Option<TicketRecord>> {
        match self {
            Self::Memory(store) => Ok(store.get_by_channel(guild_id, channel_id)),
            Self::Postgres(store) => Ok(store.get_by_channel(guild_id, channel_id).await?),
        }
    }

    pub async fn list_tickets(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> Result<Vec<TicketRecord>> {
        match self {
            Self::Memory(store) => {
                Ok(store.list_tickets(guild_id, creator_user_id, include_all, limit))
            }
            Self::Postgres(store) => Ok(store
                .list_tickets(guild_id, creator_user_id, include_all, limit)
                .await?),
        }
    }

    pub async fn claim_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.claim_ticket(ticket_id, actor_user_id)),
            Self::Postgres(store) => Ok(store.claim_ticket(ticket_id, actor_user_id).await?),
        }
    }

    pub async fn unclaim_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        allow_any: bool,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => {
                Ok(store.unclaim_ticket(ticket_id, actor_user_id, allow_any))
            }
            Self::Postgres(store) => Ok(store
                .unclaim_ticket(ticket_id, actor_user_id, allow_any)
                .await?),
        }
    }

    pub async fn close_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        reason: Option<&str>,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.close_ticket(ticket_id, actor_user_id, reason)),
            Self::Postgres(store) => Ok(store
                .close_ticket(ticket_id, actor_user_id, reason)
                .await?),
        }
    }

    pub async fn reopen_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.reopen_ticket(ticket_id, actor_user_id)),
            Self::Postgres(store) => Ok(store.reopen_ticket(ticket_id, actor_user_id).await?),
        }
    }

    pub async fn rename_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        channel_name: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => {
                Ok(store.rename_ticket(ticket_id, actor_user_id, channel_name))
            }
            Self::Postgres(store) => Ok(store
                .rename_ticket(ticket_id, actor_user_id, channel_name)
                .await?),
        }
    }

    pub async fn set_priority(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        priority: &str,
    ) -> Result<TicketMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.set_priority(ticket_id, actor_user_id, priority)),
            Self::Postgres(store) => Ok(store
                .set_priority(ticket_id, actor_user_id, priority)
                .await?),
        }
    }

    pub async fn add_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> Result<ParticipantMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.add_participant(ticket_id, user_id, actor_user_id)),
            Self::Postgres(store) => Ok(store
                .add_participant(ticket_id, user_id, actor_user_id)
                .await?),
        }
    }

    pub async fn remove_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> Result<ParticipantMutationOutcome> {
        match self {
            Self::Memory(store) => Ok(store.remove_participant(
                ticket_id,
                user_id,
                actor_user_id,
            )),
            Self::Postgres(store) => Ok(store
                .remove_participant(ticket_id, user_id, actor_user_id)
                .await?),
        }
    }

    pub async fn list_participants(&self, ticket_id: &str) -> Result<Vec<TicketParticipant>> {
        match self {
            Self::Memory(store) => Ok(store.list_participants(ticket_id)),
            Self::Postgres(store) => Ok(store.list_participants(ticket_id).await?),
        }
    }

    pub async fn store_transcript(
        &self,
        transcript: NewTicketTranscript,
    ) -> Result<TicketTranscript> {
        match self {
            Self::Memory(store) => Ok(store.store_transcript(transcript)),
            Self::Postgres(store) => Ok(store.store_transcript(transcript).await?),
        }
    }

    pub async fn list_events(&self, ticket_id: &str, limit: u16) -> Result<Vec<TicketEvent>> {
        match self {
            Self::Memory(store) => {
                Ok(store.list_events(ticket_id, usize::from(limit)))
            }
            Self::Postgres(store) => Ok(store.list_events(ticket_id, limit).await?),
        }
    }
}

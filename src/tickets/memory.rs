use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use chrono::{Duration, Utc};
use serde_json::{Value, json};

use super::model::{
    NewTicketReservation, NewTicketTranscript, ParticipantMutationOutcome, ReserveTicketOutcome,
    STATUS_CLAIMED, STATUS_CLOSED, STATUS_FAILED, STATUS_OPEN, STATUS_PROVISIONING, TicketEvent,
    TicketMutation, TicketMutationOutcome, TicketParticipant, TicketRecord, TicketTranscript,
};

const PROVISIONING_TIMEOUT_MINUTES: i64 = 10;

#[derive(Default)]
struct MemoryTicketState {
    next_number: i64,
    next_event_id: i64,
    tickets: HashMap<String, TicketRecord>,
    participants: HashMap<String, BTreeMap<String, TicketParticipant>>,
    transcripts: Vec<TicketTranscript>,
    events: Vec<TicketEvent>,
}

#[derive(Clone, Default)]
pub struct MemoryTicketStore {
    state: Arc<Mutex<MemoryTicketState>>,
}

impl MemoryTicketStore {
    pub fn reserve_ticket(
        &self,
        reservation: NewTicketReservation,
        max_open_per_user: u8,
    ) -> ReserveTicketOutcome {
        let mut state = self.lock();
        let now = Utc::now();
        expire_stale_provisioning(&mut state, now);

        let active_count = state
            .tickets
            .values()
            .filter(|ticket| {
                ticket.guild_id == reservation.guild_id
                    && ticket.creator_user_id == reservation.creator_user_id
                    && ticket.is_active()
            })
            .count();
        if active_count >= usize::from(max_open_per_user) {
            return ReserveTicketOutcome::LimitReached {
                limit: max_open_per_user,
                active_count: u8::try_from(active_count).unwrap_or(u8::MAX),
            };
        }

        state.next_number = state.next_number.max(0).saturating_add(1);
        let ticket = reservation.materialize(state.next_number, now);
        push_event(
            &mut state,
            &ticket.id,
            &ticket.guild_id,
            Some(&ticket.creator_user_id),
            "ticket_reserved",
            json!({
                "number": ticket.number,
                "queue": ticket.queue,
                "subject": ticket.subject,
            }),
        );
        state.tickets.insert(ticket.id.clone(), ticket.clone());
        ReserveTicketOutcome::Reserved(Box::new(ticket))
    }

    pub fn activate_ticket(
        &self,
        ticket_id: &str,
        channel_id: &str,
        channel_name: &str,
    ) -> TicketMutationOutcome {
        let mut state = self.lock();
        if state.tickets.values().any(|ticket| {
            ticket.id != ticket_id
                && ticket.channel_id.as_deref() == Some(channel_id)
                && ticket.status != STATUS_FAILED
        }) {
            return TicketMutationOutcome::Conflict {
                message: "this Discord channel is already linked to another ticket".to_owned(),
            };
        }

        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if ticket.status != STATUS_PROVISIONING {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            ticket.activate(channel_id.to_owned(), channel_name.to_owned(), Utc::now());
            ticket.clone()
        };

        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(&updated.creator_user_id),
            "ticket_opened",
            json!({
                "channel_id": channel_id,
                "channel_name": channel_name,
            }),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn fail_provisioning(&self, ticket_id: &str, error: &str) -> TicketMutationOutcome {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if ticket.status != STATUS_PROVISIONING {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            ticket.fail_provisioning(error, Utc::now());
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(&updated.creator_user_id),
            "ticket_provisioning_failed",
            json!({ "error": updated.provisioning_error }),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn get_by_channel(&self, guild_id: &str, channel_id: &str) -> Option<TicketRecord> {
        self.lock()
            .tickets
            .values()
            .find(|ticket| {
                ticket.guild_id == guild_id && ticket.channel_id.as_deref() == Some(channel_id)
            })
            .cloned()
    }

    pub fn list_tickets(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> Vec<TicketRecord> {
        let state = self.lock();
        let mut tickets = state
            .tickets
            .values()
            .filter(|ticket| {
                ticket.guild_id == guild_id
                    && ticket.is_active()
                    && (include_all || ticket.creator_user_id == creator_user_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        tickets.sort_by(|left, right| {
            priority_rank(&right.priority)
                .cmp(&priority_rank(&left.priority))
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.number.cmp(&right.number))
        });
        tickets.truncate(usize::from(limit));
        tickets
    }

    pub fn claim_ticket(&self, ticket_id: &str, actor_user_id: &str) -> TicketMutationOutcome {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            match ticket.status.as_str() {
                STATUS_OPEN => ticket.claim(actor_user_id, Utc::now()),
                STATUS_CLAIMED if ticket.claimed_by_user_id.as_deref() == Some(actor_user_id) => {
                    return TicketMutationOutcome::Conflict {
                        message: "ticket is already claimed by this user".to_owned(),
                    };
                }
                STATUS_CLAIMED => {
                    return TicketMutationOutcome::Conflict {
                        message: "ticket is already claimed by another staff member".to_owned(),
                    };
                }
                _ => {
                    return TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    };
                }
            }
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(actor_user_id),
            TicketMutation::Claim.event_name(),
            json!({ "claimed_by_user_id": actor_user_id }),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn unclaim_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        allow_any: bool,
    ) -> TicketMutationOutcome {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if ticket.status != STATUS_CLAIMED {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            if !allow_any && ticket.claimed_by_user_id.as_deref() != Some(actor_user_id) {
                return TicketMutationOutcome::Forbidden;
            }
            ticket.unclaim(Utc::now());
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(actor_user_id),
            TicketMutation::Unclaim.event_name(),
            json!({}),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn close_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        reason: Option<&str>,
    ) -> TicketMutationOutcome {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if !ticket.is_open() {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            ticket.close(actor_user_id, reason, Utc::now());
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(actor_user_id),
            TicketMutation::Close.event_name(),
            json!({ "reason": reason }),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn reopen_ticket(&self, ticket_id: &str, actor_user_id: &str) -> TicketMutationOutcome {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if ticket.status != STATUS_CLOSED {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            ticket.reopen(Utc::now());
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(actor_user_id),
            TicketMutation::Reopen.event_name(),
            json!({}),
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    pub fn rename_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        channel_name: &str,
    ) -> TicketMutationOutcome {
        self.update_open_ticket(
            ticket_id,
            actor_user_id,
            TicketMutation::Rename,
            json!({ "channel_name": channel_name }),
            |ticket| ticket.rename(channel_name.to_owned(), Utc::now()),
        )
    }

    pub fn set_priority(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        priority: &str,
    ) -> TicketMutationOutcome {
        self.update_open_ticket(
            ticket_id,
            actor_user_id,
            TicketMutation::SetPriority,
            json!({ "priority": priority }),
            |ticket| ticket.set_priority(priority, Utc::now()),
        )
    }

    pub fn add_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> ParticipantMutationOutcome {
        let mut state = self.lock();
        let (guild_id, creator_user_id, status) = match state.tickets.get(ticket_id) {
            Some(ticket) => (
                ticket.guild_id.clone(),
                ticket.creator_user_id.clone(),
                ticket.status.clone(),
            ),
            None => return ParticipantMutationOutcome::NotFound,
        };
        if !matches!(status.as_str(), STATUS_OPEN | STATUS_CLAIMED) {
            return ParticipantMutationOutcome::InvalidState {
                current_status: status,
            };
        }
        if creator_user_id == user_id {
            return ParticipantMutationOutcome::CreatorProtected;
        }

        let participants = state.participants.entry(ticket_id.to_owned()).or_default();
        if participants.contains_key(user_id) {
            return ParticipantMutationOutcome::AlreadyPresent;
        }
        let participant = TicketParticipant {
            ticket_id: ticket_id.to_owned(),
            user_id: user_id.to_owned(),
            added_by_user_id: actor_user_id.to_owned(),
            created_at: Utc::now(),
        };
        participants.insert(user_id.to_owned(), participant.clone());
        push_event(
            &mut state,
            ticket_id,
            &guild_id,
            Some(actor_user_id),
            "ticket_participant_added",
            json!({ "user_id": user_id }),
        );
        ParticipantMutationOutcome::Added(participant)
    }

    pub fn remove_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> ParticipantMutationOutcome {
        let mut state = self.lock();
        let (guild_id, creator_user_id, status) = match state.tickets.get(ticket_id) {
            Some(ticket) => (
                ticket.guild_id.clone(),
                ticket.creator_user_id.clone(),
                ticket.status.clone(),
            ),
            None => return ParticipantMutationOutcome::NotFound,
        };
        if !matches!(status.as_str(), STATUS_OPEN | STATUS_CLAIMED) {
            return ParticipantMutationOutcome::InvalidState {
                current_status: status,
            };
        }
        if creator_user_id == user_id {
            return ParticipantMutationOutcome::CreatorProtected;
        }

        let Some(participants) = state.participants.get_mut(ticket_id) else {
            return ParticipantMutationOutcome::NotFound;
        };
        if participants.remove(user_id).is_none() {
            return ParticipantMutationOutcome::NotFound;
        }
        push_event(
            &mut state,
            ticket_id,
            &guild_id,
            Some(actor_user_id),
            "ticket_participant_removed",
            json!({ "user_id": user_id }),
        );
        ParticipantMutationOutcome::Removed
    }

    pub fn list_participants(&self, ticket_id: &str) -> Vec<TicketParticipant> {
        self.lock()
            .participants
            .get(ticket_id)
            .map(|participants| participants.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn store_transcript(&self, transcript: NewTicketTranscript) -> TicketTranscript {
        let mut state = self.lock();
        let transcript = transcript.materialize(Utc::now());
        push_event(
            &mut state,
            &transcript.ticket_id,
            &transcript.guild_id,
            Some(&transcript.generated_by_user_id),
            "ticket_transcript_created",
            json!({
                "transcript_id": transcript.id,
                "message_count": transcript.message_count,
            }),
        );
        state.transcripts.push(transcript.clone());
        transcript
    }

    pub fn list_events(&self, ticket_id: &str, limit: usize) -> Vec<TicketEvent> {
        let state = self.lock();
        let mut events = state
            .events
            .iter()
            .filter(|event| event.ticket_id == ticket_id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| right.id.cmp(&left.id));
        events.truncate(limit);
        events
    }

    fn update_open_ticket<F>(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        mutation: TicketMutation,
        data: Value,
        update: F,
    ) -> TicketMutationOutcome
    where
        F: FnOnce(&mut TicketRecord),
    {
        let mut state = self.lock();
        let updated = {
            let Some(ticket) = state.tickets.get_mut(ticket_id) else {
                return TicketMutationOutcome::NotFound;
            };
            if !ticket.is_open() {
                return TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                };
            }
            update(ticket);
            ticket.clone()
        };
        push_event(
            &mut state,
            &updated.id,
            &updated.guild_id,
            Some(actor_user_id),
            mutation.event_name(),
            data,
        );
        TicketMutationOutcome::Updated(Box::new(updated))
    }

    fn lock(&self) -> MutexGuard<'_, MemoryTicketState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn expire_stale_provisioning(state: &mut MemoryTicketState, now: chrono::DateTime<Utc>) {
    let stale_before = now - Duration::minutes(PROVISIONING_TIMEOUT_MINUTES);
    let stale_ids = state
        .tickets
        .values()
        .filter(|ticket| ticket.status == STATUS_PROVISIONING && ticket.created_at < stale_before)
        .map(|ticket| ticket.id.clone())
        .collect::<Vec<_>>();

    for ticket_id in stale_ids {
        let updated = {
            let Some(ticket) = state.tickets.get_mut(&ticket_id) else {
                continue;
            };
            ticket.fail_provisioning("ticket provisioning lease expired", now);
            ticket.clone()
        };
        push_event(
            state,
            &updated.id,
            &updated.guild_id,
            None,
            "ticket_provisioning_expired",
            json!({}),
        );
    }
}

fn push_event(
    state: &mut MemoryTicketState,
    ticket_id: &str,
    guild_id: &str,
    actor_user_id: Option<&str>,
    event_type: &str,
    data: Value,
) {
    state.next_event_id = state.next_event_id.saturating_add(1);
    state.events.push(TicketEvent {
        id: state.next_event_id,
        ticket_id: ticket_id.to_owned(),
        guild_id: guild_id.to_owned(),
        actor_user_id: actor_user_id.map(str::to_owned),
        event_type: event_type.to_owned(),
        data,
        created_at: Utc::now(),
    });
}

fn priority_rank(priority: &str) -> u8 {
    match priority {
        "urgent" => 4,
        "high" => 3,
        "normal" => 2,
        "low" => 1,
        _ => 0,
    }
}

use std::path::PathBuf;

use chrono::Utc;
use serde_json::json;
use zuckerbot::{
    lua::{LuaAction, LuaEngine, LuaExecutionContext, LuaLimits, TicketOperation},
    tickets::{
        NewTicketReservation, NewTicketTranscript, ParticipantMutationOutcome,
        ReserveTicketOutcome, STATUS_CLAIMED, STATUS_CLOSED, STATUS_OPEN, TicketMutationOutcome,
        TicketStore,
    },
};

fn limits() -> LuaLimits {
    LuaLimits {
        memory_bytes: 4 * 1024 * 1024,
        instruction_limit: 300_000,
        hook_granularity: 100,
    }
}

fn engine() -> LuaEngine {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    LuaEngine::load(scripts, limits()).expect("bundled Lua modules should load")
}

fn lua_context(options: serde_json::Value) -> LuaExecutionContext {
    LuaExecutionContext {
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: "100000000000000002".to_owned(),
        user_id: "100000000000000003".to_owned(),
        user_name: "Tester".to_owned(),
        member_roles: Vec::new(),
        member_permissions: "0".to_owned(),
        locale: "pl".to_owned(),
        options,
        config: json!({
            "open_category_id": "100000000000000010",
            "archive_category_id": "100000000000000011",
            "log_channel_id": "100000000000000012",
            "support_role_ids": ["100000000000000013"],
            "allowed_queues": ["support", "billing"],
            "default_queue": "support",
            "max_open_per_user": 3,
            "channel_name_prefix": "ticket",
            "creator_can_close": true,
            "generate_transcript_on_close": true,
            "transcript_max_messages": 500
        }),
    }
}

fn reservation(creator_user_id: &str) -> NewTicketReservation {
    NewTicketReservation {
        guild_id: "100000000000000001".to_owned(),
        creator_user_id: creator_user_id.to_owned(),
        subject: "Problem z kontem".to_owned(),
        description: "Nie mogę otworzyć panelu ustawień.".to_owned(),
        queue: "support".to_owned(),
    }
}

async fn reserved_ticket(
    store: &TicketStore,
    creator_user_id: &str,
    limit: u8,
) -> zuckerbot::tickets::TicketRecord {
    match store
        .reserve_ticket(reservation(creator_user_id), limit)
        .await
        .expect("memory ticket reservation should succeed")
    {
        ReserveTicketOutcome::Reserved(ticket) => *ticket,
        ReserveTicketOutcome::LimitReached { limit, .. } => {
            panic!("unexpected ticket quota {limit}")
        }
    }
}

async fn active_ticket(store: &TicketStore) -> zuckerbot::tickets::TicketRecord {
    let ticket = reserved_ticket(store, "100000000000000003", 3).await;
    match store
        .activate_ticket(
            &ticket.id,
            "100000000000000020",
            "ticket-000001-problem-z-kontem",
        )
        .await
        .expect("memory ticket activation should succeed")
    {
        TicketMutationOutcome::Updated(ticket) => *ticket,
        outcome => panic!("unexpected activation outcome: {outcome:?}"),
    }
}

#[tokio::test]
async fn lua_open_subcommand_emits_a_valid_ticket_action() {
    let engine = engine();
    assert_eq!(
        engine.module_for_command("ticket").await.as_deref(),
        Some("tickets")
    );

    let actions = engine
        .execute_command(
            "tickets",
            "ticket",
            lua_context(json!({
                "open": {
                    "subject": "Płatność",
                    "description": "Płatność została pobrana dwa razy.",
                    "queue": "billing"
                }
            })),
        )
        .await
        .expect("ticket open should execute in the Lua sandbox");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::Ticket {
            operation: TicketOperation::Open {
                queue,
                policy,
                ..
            }
        }) if queue == "billing"
            && policy.open_category_id == "100000000000000010"
            && policy.support_role_ids == ["100000000000000013"]
    ));
}

#[tokio::test]
async fn lua_rejects_a_queue_outside_server_configuration() {
    let engine = engine();
    let actions = engine
        .execute_command(
            "tickets",
            "ticket",
            lua_context(json!({
                "open": {
                    "subject": "Inne",
                    "description": "Test niedozwolonej kolejki.",
                    "queue": "secret"
                }
            })),
        )
        .await
        .expect("Lua validation response should execute");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::Reply {
            ephemeral: true,
            content,
        }) if content.contains("nie jest dozwolona")
    ));
}

#[tokio::test]
async fn per_user_open_ticket_quota_is_enforced_atomically_by_the_store_contract() {
    let store = TicketStore::memory();
    reserved_ticket(&store, "100000000000000003", 1).await;

    let outcome = store
        .reserve_ticket(reservation("100000000000000003"), 1)
        .await
        .expect("second reservation should return a quota outcome");
    assert!(matches!(
        outcome,
        ReserveTicketOutcome::LimitReached {
            limit: 1,
            active_count: 1,
        }
    ));
}

#[tokio::test]
async fn ticket_claim_close_and_reopen_preserve_the_lifecycle_contract() {
    let store = TicketStore::memory();
    let ticket = active_ticket(&store).await;
    assert_eq!(ticket.status, STATUS_OPEN);

    let claimed = store
        .claim_ticket(&ticket.id, "100000000000000030")
        .await
        .expect("claim should succeed");
    let TicketMutationOutcome::Updated(claimed) = claimed else {
        panic!("expected updated claim outcome")
    };
    assert_eq!(claimed.status, STATUS_CLAIMED);
    assert_eq!(
        claimed.claimed_by_user_id.as_deref(),
        Some("100000000000000030")
    );

    let closed = store
        .close_ticket(
            &ticket.id,
            "100000000000000030",
            Some("Rozwiązano"),
        )
        .await
        .expect("close should succeed");
    let TicketMutationOutcome::Updated(closed) = closed else {
        panic!("expected updated close outcome")
    };
    assert_eq!(closed.status, STATUS_CLOSED);
    assert_eq!(closed.close_reason.as_deref(), Some("Rozwiązano"));

    let reopened = store
        .reopen_ticket(&ticket.id, "100000000000000031")
        .await
        .expect("reopen should succeed");
    let TicketMutationOutcome::Updated(reopened) = reopened else {
        panic!("expected updated reopen outcome")
    };
    assert_eq!(reopened.status, STATUS_OPEN);
    assert!(reopened.closed_at.is_none());
}

#[tokio::test]
async fn creator_cannot_be_added_or_removed_as_a_regular_participant() {
    let store = TicketStore::memory();
    let ticket = active_ticket(&store).await;

    let add = store
        .add_participant(
            &ticket.id,
            &ticket.creator_user_id,
            "100000000000000030",
        )
        .await
        .expect("participant mutation should return a domain outcome");
    assert!(matches!(
        add,
        ParticipantMutationOutcome::CreatorProtected
    ));

    let remove = store
        .remove_participant(
            &ticket.id,
            &ticket.creator_user_id,
            "100000000000000030",
        )
        .await
        .expect("participant mutation should return a domain outcome");
    assert!(matches!(
        remove,
        ParticipantMutationOutcome::CreatorProtected
    ));
}

#[tokio::test]
async fn participant_and_transcript_changes_are_persisted_with_ticket_events() {
    let store = TicketStore::memory();
    let ticket = active_ticket(&store).await;

    let participant = store
        .add_participant(
            &ticket.id,
            "100000000000000040",
            "100000000000000030",
        )
        .await
        .expect("participant add should succeed");
    assert!(matches!(
        participant,
        ParticipantMutationOutcome::Added(_)
    ));
    assert_eq!(
        store
            .list_participants(&ticket.id)
            .await
            .expect("participant list should succeed")
            .len(),
        1
    );

    let transcript = store
        .store_transcript(NewTicketTranscript {
            ticket_id: ticket.id.clone(),
            guild_id: ticket.guild_id.clone(),
            channel_id: ticket
                .channel_id
                .clone()
                .expect("active ticket should have a channel"),
            generated_by_user_id: "100000000000000030".to_owned(),
            message_count: 2,
            content: "[message one]\n[message two]\n".to_owned(),
        })
        .await
        .expect("transcript storage should succeed");
    assert_eq!(transcript.message_count, 2);
    assert_eq!(transcript.ticket_id, ticket.id);

    let events = store
        .list_events(&ticket.id, 20)
        .await
        .expect("event history should succeed");
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "ticket_participant_added")
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == "ticket_transcript_created")
    );
    assert!(events.iter().any(|event| event.created_at <= Utc::now()));
}

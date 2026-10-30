use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;
use zuckerbot::{
    moderation_cases::{CaseKind, CaseSeverity, CreateCase, ModerationCaseStore},
    scheduler::{
        CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, SchedulerStore,
    },
    session::{DashboardGuild, DashboardUser, PostgresSessionStore},
    storage::{PostgresStore, UpdateModuleSettingsParams},
    tickets::{
        NewTicketReservation, PostgresTicketStore, ReserveTicketOutcome, STATUS_OPEN,
        TicketMutationOutcome,
    },
};

fn get_database_url() -> Option<String> {
    std::env::var("DATABASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

#[tokio::test]
async fn test_postgres_integration_suite() {
    let Some(database_url) = get_database_url() else {
        eprintln!("SKIPPING PostgreSQL integration tests: DATABASE_URL not set");
        return;
    };

    println!("Running PostgreSQL integration tests against {database_url}");

    // 1. Verify migrations and storage connectivity
    let store = PostgresStore::connect(&database_url)
        .await
        .expect("PostgresStore should connect and run all migrations");
    store.ping().await.expect("ping should succeed");

    let guild_id = format!("pg_test_guild_{}", Uuid::new_v4().simple());
    let module_id = "test_module";

    // 2. Test transactional settings & audit event
    let initial = store
        .get_module_settings(&guild_id, module_id)
        .await
        .unwrap();
    assert!(initial.is_none());

    let params = UpdateModuleSettingsParams {
        guild_id: &guild_id,
        module_id,
        enabled: true,
        config: json!({"theme": "dark", "rate": 5}),
        actor_id: Some("actor_123"),
        audit_event: "module_config_updated",
        audit_data: json!({"reason": "integration_test"}),
    };

    let updated = store
        .update_module_settings_with_audit(params)
        .await
        .expect("atomic settings update and audit should succeed in transaction");
    assert!(updated.enabled);
    assert_eq!(updated.config["theme"], "dark");

    let fetched = store
        .get_module_settings(&guild_id, module_id)
        .await
        .unwrap()
        .expect("module settings should persist in PostgreSQL");
    assert!(fetched.enabled);
    assert_eq!(fetched.config["rate"], 5);

    // 3. Test Scheduler Store in PostgreSQL
    let scheduler = SchedulerStore::connect(Some(&database_url))
        .await
        .expect("SchedulerStore should connect to Postgres");

    let now = Utc::now();
    let job = NewScheduledJob {
        guild_id: guild_id.clone(),
        module_id: "scheduler".to_owned(),
        channel_id: "channel_1001".to_owned(),
        creator_user_id: "user_owner_1".to_owned(),
        content: "PostgreSQL test reminder".to_owned(),
        mention_creator: true,
        run_at: now - Duration::seconds(5), // already due
        repeat_every_seconds: None,
        remaining_runs: Some(1),
        max_attempts: 5,
    };

    let outcome = scheduler
        .create_job(job, 10)
        .await
        .expect("create job in postgres");
    let job_record = match outcome {
        CreateJobOutcome::Created(record) => record,
        other => panic!("expected Created job outcome, got {:?}", other),
    };

    // List jobs
    let jobs = scheduler
        .list_jobs(&guild_id, "user_owner_1", true, 10)
        .await
        .expect("list jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, job_record.id);

    // Mutate job (Pause and Cancel)
    let pause_outcome = scheduler
        .mutate_job(
            &guild_id,
            &job_record.id,
            "user_owner_1",
            false,
            JobMutation::Pause,
        )
        .await
        .expect("pause job");
    assert!(matches!(pause_outcome, JobMutationOutcome::Updated(_)));

    let cancel_outcome = scheduler
        .mutate_job(
            &guild_id,
            &job_record.id,
            "user_owner_1",
            false,
            JobMutation::Cancel,
        )
        .await
        .expect("cancel job");
    assert!(matches!(cancel_outcome, JobMutationOutcome::Updated(_)));

    // 4. Test Tickets Store in PostgreSQL
    let tickets = PostgresTicketStore::connect(&database_url)
        .await
        .expect("TicketStore should connect to Postgres");

    let reservation = NewTicketReservation {
        guild_id: guild_id.clone(),
        creator_user_id: "ticket_creator_1".to_owned(),
        subject: "Problem z kontem".to_owned(),
        description: "Nie mogę otworzyć panelu.".to_owned(),
        queue: "support".to_owned(),
    };

    let reserve_outcome = tickets
        .reserve_ticket(reservation.clone(), 1)
        .await
        .expect("reserve ticket in postgres");
    let reservation_record = match reserve_outcome {
        ReserveTicketOutcome::Reserved(record) => *record,
        other => panic!("expected Reserved outcome, got {:?}", other),
    };

    // Attempt second open ticket with limit 1 should be rejected atomically
    let limit_outcome = tickets
        .reserve_ticket(reservation, 1)
        .await
        .expect("second reserve check");
    match limit_outcome {
        ReserveTicketOutcome::LimitReached {
            limit,
            active_count,
        } => {
            assert_eq!(limit, 1);
            assert_eq!(active_count, 1);
        }
        other => panic!("expected LimitReached, got {:?}", other),
    }

    // Activate ticket
    let act_outcome = tickets
        .activate_ticket(&reservation_record.id, "channel_999", "ticket-support-1")
        .await
        .expect("activate ticket");
    match act_outcome {
        TicketMutationOutcome::Updated(rec) => {
            assert_eq!(rec.status, STATUS_OPEN);
            assert_eq!(rec.channel_id.as_deref(), Some("channel_999"));
        }
        other => panic!("expected Updated outcome, got {:?}", other),
    }

    // 5. Test Sessions Store in PostgreSQL
    let sessions = PostgresSessionStore::connect(&database_url)
        .await
        .expect("PostgresSessionStore should connect to Postgres");

    let session_id = format!("pg_sess_token_{}", Uuid::new_v4());
    let user = DashboardUser {
        id: "pg_user_42".to_string(),
        username: "postgres_admin".to_string(),
        global_name: Some("Postgres Admin".to_string()),
        avatar: None,
    };
    let guilds = vec![DashboardGuild {
        id: guild_id.clone(),
        name: "PostgreSQL Server".to_string(),
        icon: None,
        owner: true,
        permissions: "8".to_string(),
    }];

    sessions
        .save_session(
            &session_id,
            user.clone(),
            guilds.clone(),
            "pg_csrf_secret".to_string(),
            Utc::now() + Duration::hours(1),
            Some("ephemeral_discord_bearer_token".to_string()),
        )
        .await
        .expect("save session to postgres");

    // Fetch session and verify access token was discarded
    let fetched_session = sessions
        .get_session(&session_id)
        .await
        .unwrap()
        .expect("session should exist in postgres");
    assert_eq!(fetched_session.user.id, "pg_user_42");
    assert_eq!(fetched_session.user.username, "postgres_admin");
    assert_eq!(fetched_session.guilds.len(), 1);
    assert!(
        fetched_session.access_token.is_none(),
        "access_token must not be persisted at rest"
    );

    // Test OAuth state single-use consumption
    let oauth_token = format!("oauth_state_{}", Uuid::new_v4());
    sessions.save_oauth_state(&oauth_token).await.unwrap();
    assert!(sessions.consume_oauth_state(&oauth_token).await.unwrap());
    assert!(!sessions.consume_oauth_state(&oauth_token).await.unwrap());

    // Test session logout deletion
    sessions.delete_session(&session_id).await.unwrap();
    assert!(sessions.get_session(&session_id).await.unwrap().is_none());

    // 6. Test ModerationCaseStore and Audit Log in PostgreSQL
    let mod_store = ModerationCaseStore::connect(Some(&database_url))
        .await
        .expect("ModerationCaseStore should connect to Postgres");
    let mod_now = Utc::now();
    let mod_case = mod_store
        .create_case(
            CreateCase {
                guild_id: 9990001,
                subject_user_id: 1110001,
                actor_user_id: 2220001,
                kind: CaseKind::Warning,
                severity: CaseSeverity::High,
                points: 5,
                reason: "PostgreSQL integration test warning".to_string(),
                source_module: "moderation".to_string(),
                visible_to_subject: true,
                expires_at: Some(mod_now + Duration::days(7)),
            },
            mod_now,
        )
        .await
        .expect("create moderation case in PostgreSQL");
    assert!(mod_case.case_number >= 1);
    assert_eq!(mod_case.points, 5);

    mod_store
        .add_note(
            9990001,
            mod_case.case_number,
            2220001,
            "Postgres test note".to_string(),
            true,
            mod_now,
        )
        .await
        .expect("add note in PostgreSQL");

    let details = mod_store
        .get_case(9990001, mod_case.case_number, mod_now)
        .await
        .expect("get case details from PostgreSQL");
    assert_eq!(details.notes.len(), 1);

    // Verify Audit log persistence and retrieval
    let audit_events = store.list_audit_events(&guild_id, 10).await.unwrap();
    assert!(!audit_events.is_empty());
    assert_eq!(audit_events[0].module_id, "test_module");
}

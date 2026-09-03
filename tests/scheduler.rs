use std::path::PathBuf;

use chrono::{Duration, Utc};
use serde_json::json;
use zuckerbot::{
    lua::{
        LuaAction, LuaEngine, LuaExecutionContext, LuaLimits, SchedulerOperation,
    },
    scheduler::{
        CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, STATUS_ACTIVE,
        STATUS_COMPLETED, STATUS_PROCESSING, SchedulerStore,
    },
};

fn limits() -> LuaLimits {
    LuaLimits {
        memory_bytes: 4 * 1024 * 1024,
        instruction_limit: 200_000,
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
            "user_reminders_enabled": true,
            "max_user_jobs": 10,
            "minimum_delay_seconds": 30,
            "maximum_delay_days": 365,
            "allow_user_mentions": true
        }),
    }
}

fn new_job(run_at: chrono::DateTime<Utc>) -> NewScheduledJob {
    NewScheduledJob {
        guild_id: "100000000000000001".to_owned(),
        module_id: "scheduler".to_owned(),
        channel_id: "100000000000000002".to_owned(),
        creator_user_id: "100000000000000003".to_owned(),
        content: "Test reminder".to_owned(),
        mention_creator: true,
        run_at,
        repeat_every_seconds: None,
        remaining_runs: Some(1),
        max_attempts: 5,
    }
}

async fn created_job(
    store: &SchedulerStore,
    job: NewScheduledJob,
    max_jobs: u16,
) -> zuckerbot::scheduler::ScheduledJob {
    match store
        .create_job(job, max_jobs)
        .await
        .expect("memory scheduler create should succeed")
    {
        CreateJobOutcome::Created(job) => job,
        CreateJobOutcome::LimitReached { limit } => {
            panic!("unexpected scheduler limit {limit}")
        }
    }
}

#[tokio::test]
async fn lua_reminder_emits_a_valid_scheduler_action() {
    let engine = engine();
    assert_eq!(
        engine.module_for_command("remind").await.as_deref(),
        Some("scheduler")
    );

    let actions = engine
        .execute_command(
            "scheduler",
            "remind",
            lua_context(json!({
                "when": "15m",
                "message": "Sprawdź wdrożenie",
                "mention": true
            })),
        )
        .await
        .expect("remind command should execute in the sandbox");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::Scheduler {
            operation: SchedulerOperation::Create {
                when,
                mention_creator: true,
                ..
            }
        }) if when == "15m"
    ));
}

#[tokio::test]
async fn per_user_pending_job_limit_is_enforced() {
    let store = SchedulerStore::memory();
    let first = new_job(Utc::now() + Duration::minutes(5));
    created_job(&store, first.clone(), 1).await;

    let outcome = store
        .create_job(first, 1)
        .await
        .expect("memory scheduler create should succeed");
    assert!(matches!(
        outcome,
        CreateJobOutcome::LimitReached { limit: 1 }
    ));
}

#[tokio::test]
async fn lease_prevents_two_workers_from_claiming_the_same_job() {
    let store = SchedulerStore::memory();
    let now = Utc::now();
    let created = created_job(&store, new_job(now - Duration::seconds(1)), 10).await;

    let first = store
        .claim_due(
            "worker-a",
            now,
            now - Duration::seconds(60),
            25,
        )
        .await
        .expect("first claim should succeed");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].id, created.id);
    assert_eq!(first[0].status, STATUS_PROCESSING);

    let second = store
        .claim_due(
            "worker-b",
            now,
            now - Duration::seconds(60),
            25,
        )
        .await
        .expect("second claim should succeed");
    assert!(second.is_empty());
}

#[tokio::test]
async fn finite_recurring_job_reschedules_then_completes() {
    let store = SchedulerStore::memory();
    let now = Utc::now();
    let mut recurring = new_job(now - Duration::seconds(1));
    recurring.repeat_every_seconds = Some(60);
    recurring.remaining_runs = Some(2);
    let created = created_job(&store, recurring, 10).await;

    let first_claim = store
        .claim_due(
            "worker-a",
            now,
            now - Duration::seconds(60),
            1,
        )
        .await
        .expect("claim should succeed");
    assert_eq!(first_claim.len(), 1);
    let after_first = store
        .mark_succeeded(&created.id, "worker-a", now)
        .await
        .expect("completion should succeed")
        .expect("worker should still own the lease");
    assert_eq!(after_first.status, STATUS_ACTIVE);
    assert_eq!(after_first.remaining_runs, Some(1));

    let second_now = after_first.run_at + Duration::seconds(1);
    let second_claim = store
        .claim_due(
            "worker-a",
            second_now,
            second_now - Duration::seconds(60),
            1,
        )
        .await
        .expect("second claim should succeed");
    assert_eq!(second_claim.len(), 1);
    let completed = store
        .mark_succeeded(&created.id, "worker-a", second_now)
        .await
        .expect("second completion should succeed")
        .expect("worker should own the second lease");
    assert_eq!(completed.status, STATUS_COMPLETED);
    assert_eq!(completed.run_count, 2);
}

#[tokio::test]
async fn non_owner_cannot_modify_another_users_job() {
    let store = SchedulerStore::memory();
    let created = created_job(
        &store,
        new_job(Utc::now() + Duration::minutes(5)),
        10,
    )
    .await;

    let denied = store
        .mutate_job(
            &created.guild_id,
            &created.id,
            "100000000000000099",
            false,
            JobMutation::Cancel,
        )
        .await
        .expect("mutation should return an authorization outcome");
    assert!(matches!(denied, JobMutationOutcome::Forbidden));

    let allowed = store
        .mutate_job(
            &created.guild_id,
            &created.id,
            "100000000000000099",
            true,
            JobMutation::Pause,
        )
        .await
        .expect("staff mutation should succeed");
    assert!(matches!(allowed, JobMutationOutcome::Updated(_)));
}

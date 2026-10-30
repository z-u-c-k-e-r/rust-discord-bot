use chrono::{Duration, Utc};
use zuckerbot::moderation_cases::{
    CaseFilter, CaseKind, CaseSeverity, CaseStatus, CreateCase, ModerationCaseStore, UpdateCase,
};

fn warning(now: chrono::DateTime<Utc>) -> CreateCase {
    CreateCase {
        guild_id: 10,
        subject_user_id: 20,
        actor_user_id: 30,
        kind: CaseKind::Warning,
        severity: CaseSeverity::Medium,
        points: 3,
        reason: "Repeated harassment after a moderator request".to_owned(),
        source_module: "moderation".to_owned(),
        visible_to_subject: true,
        expires_at: Some(now + Duration::days(30)),
    }
}

#[tokio::test]
async fn complete_case_lifecycle_is_versioned_and_audited() {
    let store = ModerationCaseStore::memory();
    let now = Utc::now();
    let created = store.create_case(warning(now), now).await.unwrap();
    assert_eq!(created.case_number, 1);
    assert_eq!(created.version, 1);

    let note = store
        .add_note(
            10,
            created.case_number,
            30,
            "User acknowledged the warning.".to_owned(),
            true,
            now,
        )
        .await
        .unwrap();
    assert!(note.visible_to_subject);

    store
        .add_evidence(
            10,
            created.case_number,
            30,
            "message".to_owned(),
            "https://discord.com/channels/10/40/50".to_owned(),
            now,
        )
        .await
        .unwrap();

    let updated = store
        .update_case(
            10,
            created.case_number,
            30,
            1,
            UpdateCase {
                severity: Some(CaseSeverity::High),
                points: Some(5),
                ..UpdateCase::default()
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(updated.version, 2);

    let voided = store
        .void_case(
            10,
            created.case_number,
            30,
            2,
            "Appeal accepted after reviewing context.".to_owned(),
            now,
        )
        .await
        .unwrap();
    assert_eq!(voided.status, CaseStatus::Voided);

    let restored = store
        .restore_case(10, created.case_number, 30, 3, now)
        .await
        .unwrap();
    assert_eq!(restored.status, CaseStatus::Active);
    assert_eq!(restored.version, 4);

    let details = store.get_case(10, created.case_number, now).await.unwrap();
    assert_eq!(details.notes.len(), 1);
    assert_eq!(details.evidence.len(), 1);
    assert!(details.events.len() >= 6);
}

#[tokio::test]
async fn self_service_filter_never_exposes_staff_only_records() {
    let store = ModerationCaseStore::memory();
    let now = Utc::now();
    store.create_case(warning(now), now).await.unwrap();
    let mut private = warning(now);
    private.kind = CaseKind::StaffNote;
    private.visible_to_subject = false;
    private.reason = "Internal risk assessment".to_owned();
    store.create_case(private, now).await.unwrap();

    let rows = store
        .list_cases(
            10,
            CaseFilter {
                subject_user_id: Some(20),
                visible_to_subject_only: true,
                limit: 25,
                ..CaseFilter::default()
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].visible_to_subject);
}

#[tokio::test]
async fn expired_cases_stop_contributing_to_active_stats() {
    let store = ModerationCaseStore::memory();
    let now = Utc::now();
    let created = store.create_case(warning(now), now).await.unwrap();
    let later = now + Duration::days(31);
    let stats = store.stats(10, None, later).await.unwrap();
    assert_eq!(stats.total_cases, 1);
    assert_eq!(stats.active_cases, 0);
    assert_eq!(stats.expired_cases, 1);
    let details = store
        .get_case(10, created.case_number, later)
        .await
        .unwrap();
    assert_eq!(details.case.status, CaseStatus::Expired);
}

#[tokio::test]
async fn lua_moderation_module_emits_valid_case_actions() {
    use serde_json::json;
    use std::path::PathBuf;
    use zuckerbot::lua::{
        LuaAction, LuaEngine, LuaExecutionContext, LuaLimits, ModerationCaseOperation,
    };

    let engine = LuaEngine::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        LuaLimits {
            memory_bytes: 4 * 1024 * 1024,
            instruction_limit: 300_000,
            hook_granularity: 100,
        },
    )
    .expect("scripts should load");

    let ctx = LuaExecutionContext {
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: "100000000000000002".to_owned(),
        user_id: "100000000000000003".to_owned(),
        user_name: "ModUser".to_owned(),
        member_roles: Vec::new(),
        member_permissions: "8".to_owned(), // Administrator
        locale: "pl".to_owned(),
        options: json!({
            "user": "100000000000000004",
            "reason": "Test warning reason",
            "severity": "high",
            "points": 3
        }),
        config: json!({
            "default_timeout_seconds": 600
        }),
    };

    let actions = engine
        .execute_command("moderation", "warn", ctx.clone())
        .await
        .unwrap();
    assert_eq!(actions.len(), 2);
    assert!(matches!(&actions[0], LuaAction::Reply { .. }));
    let case_action = actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected ModerationCase action");

    assert!(case_action.validate().is_ok());
    match case_action {
        ModerationCaseOperation::Create {
            subject_user_id,
            kind,
            reason,
            severity,
            points,
            visible_to_subject,
            ..
        } => {
            assert_eq!(subject_user_id, "100000000000000004");
            assert_eq!(kind, "warning");
            assert_eq!(reason, "Test warning reason");
            assert_eq!(severity, "high");
            assert_eq!(*points, 3);
            assert!(visible_to_subject);
        }
        other => panic!("unexpected operation: {other:?}"),
    }

    // Test /case view
    let mut case_view_ctx = ctx.clone();
    case_view_ctx.options = json!({
        "action": "view",
        "case_number": 42
    });
    let view_actions = engine
        .execute_command("moderation", "case", case_view_ctx)
        .await
        .unwrap();
    let view_op = view_actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected view operation");
    assert!(view_op.validate().is_ok());
    assert!(matches!(
        view_op,
        ModerationCaseOperation::View { case_number: 42 }
    ));

    // Test /case void
    let mut case_void_ctx = ctx.clone();
    case_void_ctx.options = json!({
        "action": "void",
        "case_number": 42,
        "text": "Mistaken identity",
        "version": 1
    });
    let void_actions = engine
        .execute_command("moderation", "case", case_void_ctx)
        .await
        .unwrap();
    let void_op = void_actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected void operation");
    assert!(void_op.validate().is_ok());
    assert!(matches!(
        void_op,
        ModerationCaseOperation::Void {
            case_number: 42,
            expected_version: 1,
            ..
        }
    ));

    // Test /case edit
    let mut case_edit_ctx = ctx.clone();
    case_edit_ctx.options = json!({
        "action": "edit",
        "case_number": 42,
        "text": "Corrected case reason",
        "version": 2
    });
    let edit_actions = engine
        .execute_command("moderation", "case", case_edit_ctx)
        .await
        .unwrap();
    let edit_op = edit_actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected edit operation");
    assert!(edit_op.validate().is_ok());
    assert!(matches!(
        edit_op,
        ModerationCaseOperation::Edit {
            case_number: 42,
            expected_version: 2,
            ..
        }
    ));

    // Test /moderate softban
    let mut softban_ctx = ctx.clone();
    softban_ctx.options = json!({
        "action": "softban",
        "user": "5550001",
        "reason": "Raid cleanup",
        "severity": "high",
        "points": 5
    });
    let softban_actions = engine
        .execute_command("moderation", "moderate", softban_ctx)
        .await
        .unwrap();
    let softban_op = softban_actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected softban case operation");
    assert!(softban_op.validate().is_ok());
    assert!(matches!(
        softban_op,
        ModerationCaseOperation::Create {
            kind,
            reason,
            ..
        } if kind == "softban" && reason == "Raid cleanup"
    ));

    // Test /moderate unban
    let mut unban_ctx = ctx.clone();
    unban_ctx.options = json!({
        "action": "unban",
        "user": "5550001",
        "reason": "Appeal accepted"
    });
    let unban_actions = engine
        .execute_command("moderation", "moderate", unban_ctx)
        .await
        .unwrap();
    let unban_op = unban_actions
        .iter()
        .find_map(|a| match a {
            LuaAction::ModerationCase { operation } => Some(operation),
            _ => None,
        })
        .expect("expected unban case operation");
    assert!(unban_op.validate().is_ok());
    assert!(matches!(
        unban_op,
        ModerationCaseOperation::Create {
            kind,
            reason,
            ..
        } if kind == "unban" && reason == "Appeal accepted"
    ));
}

#[tokio::test]
async fn softban_case_lifecycle_and_kind_serialization() {
    let store = ModerationCaseStore::memory();
    let now = Utc::now();
    let created = store
        .create_case(
            CreateCase {
                guild_id: 10,
                subject_user_id: 20,
                actor_user_id: 30,
                kind: CaseKind::Softban,
                severity: CaseSeverity::High,
                points: 3,
                reason: "Spam raid wipe".to_owned(),
                source_module: "moderation".to_owned(),
                visible_to_subject: true,
                expires_at: None,
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(created.kind, CaseKind::Softban);
    assert_eq!(created.kind.as_str(), "softban");
    assert_eq!("softban".parse::<CaseKind>().unwrap(), CaseKind::Softban);

    let details = store.get_case(10, created.case_number, now).await.unwrap();
    assert_eq!(details.case.kind, CaseKind::Softban);

    let stats = store.stats(10, None, now).await.unwrap();
    assert_eq!(stats.total_cases, 1);
}

#[tokio::test]
async fn moderation_case_store_connect_and_audit_logging() {
    use zuckerbot::storage::Storage;

    let store = ModerationCaseStore::connect(None).await.unwrap();
    let now = Utc::now();
    let created = store.create_case(warning(now), now).await.unwrap();
    assert_eq!(created.case_number, 1);

    let storage = Storage::connect(None).await.unwrap();
    storage
        .record_audit(
            Some("10"),
            Some("30"),
            "moderation",
            "case_created",
            serde_json::json!({ "case_number": 1 }),
        )
        .await
        .unwrap();

    let logs = storage.list_audit_events("10", 10).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].event, "case_created");
    assert_eq!(logs[0].actor_id.as_deref(), Some("30"));
}

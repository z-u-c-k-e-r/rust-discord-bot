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

    let details = store
        .get_case(10, created.case_number, now)
        .await
        .unwrap();
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

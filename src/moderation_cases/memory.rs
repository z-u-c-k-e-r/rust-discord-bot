use std::{collections::BTreeMap, sync::Arc};

use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{
    CaseDetails, CaseError, CaseEvent, CaseEvidence, CaseFilter, CaseKind, CaseNote,
    CaseStatus, CreateCase, ModerationCase, ModeratorStats, UpdateCase, validate_evidence,
    validate_note, validate_void_reason,
};

#[derive(Debug, Default)]
struct MemoryState {
    counters: BTreeMap<u64, i64>,
    cases: BTreeMap<(u64, i64), ModerationCase>,
    notes: BTreeMap<Uuid, Vec<CaseNote>>,
    evidence: BTreeMap<Uuid, Vec<CaseEvidence>>,
    events: BTreeMap<Uuid, Vec<CaseEvent>>,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryModerationCaseStore {
    state: Arc<Mutex<MemoryState>>,
}

impl MemoryModerationCaseStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create_case(
        &self,
        input: CreateCase,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        input.validate(now)?;
        let mut state = self.state.lock().await;
        let counter = state.counters.entry(input.guild_id).or_insert(0);
        *counter += 1;
        let case_number = *counter;
        let moderation_case = ModerationCase {
            id: Uuid::new_v4(),
            case_number,
            guild_id: input.guild_id,
            subject_user_id: input.subject_user_id,
            actor_user_id: input.actor_user_id,
            kind: input.kind,
            status: CaseStatus::Active,
            severity: input.severity,
            points: input.points,
            reason: input.reason,
            source_module: input.source_module,
            visible_to_subject: input.visible_to_subject,
            expires_at: input.expires_at,
            voided_by_user_id: None,
            void_reason: None,
            voided_at: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };
        let event = CaseEvent {
            id: Uuid::new_v4(),
            case_id: moderation_case.id,
            actor_user_id: moderation_case.actor_user_id,
            event_type: "created".to_owned(),
            payload: json!({
                "kind": moderation_case.kind,
                "severity": moderation_case.severity,
                "points": moderation_case.points,
                "visible_to_subject": moderation_case.visible_to_subject,
            }),
            created_at: now,
        };
        state
            .events
            .entry(moderation_case.id)
            .or_default()
            .push(event);
        state.cases.insert(
            (moderation_case.guild_id, moderation_case.case_number),
            moderation_case.clone(),
        );
        Ok(moderation_case)
    }

    pub async fn get_case(
        &self,
        guild_id: u64,
        case_number: i64,
        now: DateTime<Utc>,
    ) -> Result<CaseDetails, CaseError> {
        let mut state = self.state.lock().await;
        expire_due(&mut state, guild_id, now);
        let moderation_case = state
            .cases
            .get(&(guild_id, case_number))
            .cloned()
            .ok_or(CaseError::NotFound)?;
        Ok(details(&state, moderation_case))
    }

    pub async fn list_cases(
        &self,
        guild_id: u64,
        filter: CaseFilter,
        now: DateTime<Utc>,
    ) -> Result<Vec<ModerationCase>, CaseError> {
        filter.validate()?;
        let mut state = self.state.lock().await;
        expire_due(&mut state, guild_id, now);
        let rows = state
            .cases
            .range((guild_id, i64::MIN)..=(guild_id, i64::MAX))
            .rev()
            .map(|(_, value)| value)
            .filter(|value| {
                filter
                    .subject_user_id
                    .is_none_or(|expected| value.subject_user_id == expected)
                    && filter
                        .actor_user_id
                        .is_none_or(|expected| value.actor_user_id == expected)
                    && filter.kind.is_none_or(|expected| value.kind == expected)
                    && filter
                        .status
                        .is_none_or(|expected| value.status == expected)
                    && (!filter.visible_to_subject_only || value.visible_to_subject)
            })
            .take(usize::from(filter.limit))
            .cloned()
            .collect();
        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_case(
        &self,
        guild_id: u64,
        case_number: i64,
        actor_user_id: u64,
        expected_version: i64,
        update: UpdateCase,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        update.validate(now)?;
        if update.is_empty() {
            return Err(CaseError::Validation(
                "at least one case field must be changed".to_owned(),
            ));
        }
        let mut state = self.state.lock().await;
        expire_due(&mut state, guild_id, now);
        let snapshot = {
            let moderation_case = state
                .cases
                .get_mut(&(guild_id, case_number))
                .ok_or(CaseError::NotFound)?;
            verify_version(moderation_case, expected_version)?;
            if moderation_case.status == CaseStatus::Voided {
                return Err(CaseError::Validation(
                    "voided cases must be restored before editing".to_owned(),
                ));
            }
            if let Some(reason) = update.reason {
                moderation_case.reason = reason;
            }
            if let Some(severity) = update.severity {
                moderation_case.severity = severity;
            }
            if let Some(points) = update.points {
                moderation_case.points = points;
            }
            if let Some(visible) = update.visible_to_subject {
                moderation_case.visible_to_subject = visible;
            }
            if update.clear_expiry {
                moderation_case.expires_at = None;
                if moderation_case.status == CaseStatus::Expired {
                    moderation_case.status = CaseStatus::Active;
                }
            } else if let Some(expires_at) = update.expires_at {
                moderation_case.expires_at = Some(expires_at);
                moderation_case.status = CaseStatus::Active;
            }
            moderation_case.updated_at = now;
            moderation_case.version += 1;
            moderation_case.clone()
        };
        push_event(
            &mut state,
            snapshot.id,
            actor_user_id,
            "updated",
            json!({"version": snapshot.version}),
            now,
        );
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_note(
        &self,
        guild_id: u64,
        case_number: i64,
        author_user_id: u64,
        body: String,
        visible_to_subject: bool,
        now: DateTime<Utc>,
    ) -> Result<CaseNote, CaseError> {
        validate_note(&body)?;
        let mut state = self.state.lock().await;
        let case_id = state
            .cases
            .get(&(guild_id, case_number))
            .map(|value| value.id)
            .ok_or(CaseError::NotFound)?;
        let note = CaseNote {
            id: Uuid::new_v4(),
            case_id,
            author_user_id,
            body,
            visible_to_subject,
            created_at: now,
        };
        state.notes.entry(case_id).or_default().push(note.clone());
        push_event(
            &mut state,
            case_id,
            author_user_id,
            "note_added",
            json!({
                "note_id": note.id,
                "visible_to_subject": note.visible_to_subject,
            }),
            now,
        );
        Ok(note)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_evidence(
        &self,
        guild_id: u64,
        case_number: i64,
        author_user_id: u64,
        label: String,
        value: String,
        now: DateTime<Utc>,
    ) -> Result<CaseEvidence, CaseError> {
        validate_evidence(&label, &value)?;
        let mut state = self.state.lock().await;
        let case_id = state
            .cases
            .get(&(guild_id, case_number))
            .map(|moderation_case| moderation_case.id)
            .ok_or(CaseError::NotFound)?;
        let evidence = CaseEvidence {
            id: Uuid::new_v4(),
            case_id,
            author_user_id,
            label,
            value,
            created_at: now,
        };
        state
            .evidence
            .entry(case_id)
            .or_default()
            .push(evidence.clone());
        push_event(
            &mut state,
            case_id,
            author_user_id,
            "evidence_added",
            json!({"evidence_id": evidence.id, "label": evidence.label}),
            now,
        );
        Ok(evidence)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn void_case(
        &self,
        guild_id: u64,
        case_number: i64,
        actor_user_id: u64,
        expected_version: i64,
        reason: String,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        validate_void_reason(&reason)?;
        let mut state = self.state.lock().await;
        let snapshot = {
            let moderation_case = state
                .cases
                .get_mut(&(guild_id, case_number))
                .ok_or(CaseError::NotFound)?;
            verify_version(moderation_case, expected_version)?;
            if moderation_case.status == CaseStatus::Voided {
                return Err(CaseError::Validation(
                    "moderation case is already voided".to_owned(),
                ));
            }
            moderation_case.status = CaseStatus::Voided;
            moderation_case.voided_by_user_id = Some(actor_user_id);
            moderation_case.void_reason = Some(reason.clone());
            moderation_case.voided_at = Some(now);
            moderation_case.updated_at = now;
            moderation_case.version += 1;
            moderation_case.clone()
        };
        push_event(
            &mut state,
            snapshot.id,
            actor_user_id,
            "voided",
            json!({"reason": reason, "version": snapshot.version}),
            now,
        );
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn restore_case(
        &self,
        guild_id: u64,
        case_number: i64,
        actor_user_id: u64,
        expected_version: i64,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        let mut state = self.state.lock().await;
        let snapshot = {
            let moderation_case = state
                .cases
                .get_mut(&(guild_id, case_number))
                .ok_or(CaseError::NotFound)?;
            verify_version(moderation_case, expected_version)?;
            if moderation_case.status != CaseStatus::Voided {
                return Err(CaseError::Validation(
                    "only voided moderation cases can be restored".to_owned(),
                ));
            }
            moderation_case.status = if moderation_case
                .expires_at
                .is_some_and(|expires_at| expires_at <= now)
            {
                CaseStatus::Expired
            } else {
                CaseStatus::Active
            };
            moderation_case.voided_by_user_id = None;
            moderation_case.void_reason = None;
            moderation_case.voided_at = None;
            moderation_case.updated_at = now;
            moderation_case.version += 1;
            moderation_case.clone()
        };
        push_event(
            &mut state,
            snapshot.id,
            actor_user_id,
            "restored",
            json!({"status": snapshot.status, "version": snapshot.version}),
            now,
        );
        Ok(snapshot)
    }

    pub async fn stats(
        &self,
        guild_id: u64,
        actor_user_id: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<ModeratorStats, CaseError> {
        let mut state = self.state.lock().await;
        expire_due(&mut state, guild_id, now);
        let mut result = ModeratorStats {
            guild_id,
            actor_user_id,
            total_cases: 0,
            active_cases: 0,
            expired_cases: 0,
            voided_cases: 0,
            warning_cases: 0,
            timeout_cases: 0,
            kick_cases: 0,
            ban_cases: 0,
            total_points: 0,
        };
        for moderation_case in state
            .cases
            .range((guild_id, i64::MIN)..=(guild_id, i64::MAX))
            .map(|(_, value)| value)
            .filter(|value| actor_user_id.is_none_or(|actor| value.actor_user_id == actor))
        {
            result.total_cases += 1;
            result.total_points += i64::from(moderation_case.points);
            match moderation_case.status {
                CaseStatus::Active => result.active_cases += 1,
                CaseStatus::Expired => result.expired_cases += 1,
                CaseStatus::Voided => result.voided_cases += 1,
            }
            match moderation_case.kind {
                CaseKind::Warning => result.warning_cases += 1,
                CaseKind::Timeout => result.timeout_cases += 1,
                CaseKind::Kick => result.kick_cases += 1,
                CaseKind::Ban => result.ban_cases += 1,
                CaseKind::StaffNote
                | CaseKind::Unban
                | CaseKind::Automod
                | CaseKind::Other => {},
            }
        }
        Ok(result)
    }
}

fn details(state: &MemoryState, moderation_case: ModerationCase) -> CaseDetails {
    let case_id = moderation_case.id;
    CaseDetails {
        case: moderation_case,
        notes: state.notes.get(&case_id).cloned().unwrap_or_default(),
        evidence: state.evidence.get(&case_id).cloned().unwrap_or_default(),
        events: state.events.get(&case_id).cloned().unwrap_or_default(),
    }
}

fn verify_version(
    moderation_case: &ModerationCase,
    expected_version: i64,
) -> Result<(), CaseError> {
    if moderation_case.version != expected_version {
        return Err(CaseError::VersionConflict {
            expected: expected_version,
            actual: moderation_case.version,
        });
    }
    Ok(())
}

fn expire_due(state: &mut MemoryState, guild_id: u64, now: DateTime<Utc>) {
    let due = state
        .cases
        .range((guild_id, i64::MIN)..=(guild_id, i64::MAX))
        .filter_map(|(key, value)| {
            (value.status == CaseStatus::Active
                && value.expires_at.is_some_and(|expires_at| expires_at <= now))
            .then_some(*key)
        })
        .collect::<Vec<_>>();
    for key in due {
        let snapshot = state.cases.get_mut(&key).map(|moderation_case| {
            moderation_case.status = CaseStatus::Expired;
            moderation_case.updated_at = now;
            moderation_case.version += 1;
            (
                moderation_case.id,
                moderation_case.actor_user_id,
                moderation_case.version,
            )
        });
        if let Some((case_id, actor_user_id, version)) = snapshot {
            push_event(
                state,
                case_id,
                actor_user_id,
                "expired",
                json!({"version": version}),
                now,
            );
        }
    }
}

fn push_event(
    state: &mut MemoryState,
    case_id: Uuid,
    actor_user_id: u64,
    event_type: &str,
    payload: serde_json::Value,
    now: DateTime<Utc>,
) {
    state.events.entry(case_id).or_default().push(CaseEvent {
        id: Uuid::new_v4(),
        case_id,
        actor_user_id,
        event_type: event_type.to_owned(),
        payload,
        created_at: now,
    });
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::moderation_cases::CaseSeverity;

    fn input() -> CreateCase {
        CreateCase {
            guild_id: 1,
            subject_user_id: 2,
            actor_user_id: 3,
            kind: CaseKind::Warning,
            severity: CaseSeverity::Medium,
            points: 2,
            reason: "Repeated disruptive behavior".to_owned(),
            source_module: "moderation".to_owned(),
            visible_to_subject: true,
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn assigns_monotonic_per_guild_case_numbers() {
        let store = MemoryModerationCaseStore::new();
        let now = Utc::now();
        let first = store.create_case(input(), now).await.unwrap();
        let second = store.create_case(input(), now).await.unwrap();
        assert_eq!(first.case_number, 1);
        assert_eq!(second.case_number, 2);
    }

    #[tokio::test]
    async fn rejects_stale_updates() {
        let store = MemoryModerationCaseStore::new();
        let now = Utc::now();
        let created = store.create_case(input(), now).await.unwrap();
        let update = UpdateCase {
            reason: Some("Updated reason".to_owned()),
            ..UpdateCase::default()
        };
        let changed = store
            .update_case(1, created.case_number, 3, 1, update.clone(), now)
            .await
            .unwrap();
        assert_eq!(changed.version, 2);
        assert!(matches!(
            store
                .update_case(1, created.case_number, 3, 1, update, now)
                .await,
            Err(CaseError::VersionConflict { .. })
        ));
    }

    #[tokio::test]
    async fn lazily_expires_active_cases_and_records_event() {
        let store = MemoryModerationCaseStore::new();
        let now = Utc::now();
        let mut expiring = input();
        expiring.expires_at = Some(now + Duration::minutes(1));
        let created = store.create_case(expiring, now).await.unwrap();
        let details = store
            .get_case(1, created.case_number, now + Duration::minutes(2))
            .await
            .unwrap();
        assert_eq!(details.case.status, CaseStatus::Expired);
        assert!(
            details
                .events
                .iter()
                .any(|event| event.event_type == "expired")
        );
    }
}

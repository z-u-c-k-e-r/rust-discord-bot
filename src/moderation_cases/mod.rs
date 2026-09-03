mod memory;
mod model;
mod postgres;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub use memory::MemoryModerationCaseStore;
pub use model::{
    CaseDetails, CaseError, CaseEvent, CaseEvidence, CaseFilter, CaseKind, CaseNote,
    CaseSeverity, CaseStatus, CreateCase, MAX_EVIDENCE_LABEL_LENGTH, MAX_EVIDENCE_VALUE_LENGTH,
    MAX_EXPIRY_DAYS, MAX_LIST_LIMIT, MAX_NOTE_LENGTH, MAX_POINTS, MAX_REASON_LENGTH,
    MAX_SOURCE_MODULE_LENGTH, ModerationCase, ModeratorStats, UpdateCase, validate_evidence,
    validate_note, validate_void_reason,
};
pub use postgres::PostgresModerationCaseStore;

#[derive(Debug, Clone)]
pub enum ModerationCaseStore {
    Memory(MemoryModerationCaseStore),
    Postgres(PostgresModerationCaseStore),
}

impl ModerationCaseStore {
    #[must_use]
    pub fn memory() -> Self {
        Self::Memory(MemoryModerationCaseStore::new())
    }

    #[must_use]
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(PostgresModerationCaseStore::new(pool))
    }

    pub async fn create_case(
        &self,
        input: CreateCase,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        match self {
            Self::Memory(store) => store.create_case(input, now).await,
            Self::Postgres(store) => store.create_case(input, now).await,
        }
    }

    pub async fn get_case(
        &self,
        guild_id: u64,
        case_number: i64,
        now: DateTime<Utc>,
    ) -> Result<CaseDetails, CaseError> {
        match self {
            Self::Memory(store) => store.get_case(guild_id, case_number, now).await,
            Self::Postgres(store) => store.get_case(guild_id, case_number, now).await,
        }
    }

    pub async fn list_cases(
        &self,
        guild_id: u64,
        filter: CaseFilter,
        now: DateTime<Utc>,
    ) -> Result<Vec<ModerationCase>, CaseError> {
        match self {
            Self::Memory(store) => store.list_cases(guild_id, filter, now).await,
            Self::Postgres(store) => store.list_cases(guild_id, filter, now).await,
        }
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
        match self {
            Self::Memory(store) => {
                store
                    .update_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        update,
                        now,
                    )
                    .await
            }
            Self::Postgres(store) => {
                store
                    .update_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        update,
                        now,
                    )
                    .await
            }
        }
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
        match self {
            Self::Memory(store) => {
                store
                    .add_note(
                        guild_id,
                        case_number,
                        author_user_id,
                        body,
                        visible_to_subject,
                        now,
                    )
                    .await
            }
            Self::Postgres(store) => {
                store
                    .add_note(
                        guild_id,
                        case_number,
                        author_user_id,
                        body,
                        visible_to_subject,
                        now,
                    )
                    .await
            }
        }
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
        match self {
            Self::Memory(store) => {
                store
                    .add_evidence(
                        guild_id,
                        case_number,
                        author_user_id,
                        label,
                        value,
                        now,
                    )
                    .await
            }
            Self::Postgres(store) => {
                store
                    .add_evidence(
                        guild_id,
                        case_number,
                        author_user_id,
                        label,
                        value,
                        now,
                    )
                    .await
            }
        }
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
        match self {
            Self::Memory(store) => {
                store
                    .void_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        reason,
                        now,
                    )
                    .await
            }
            Self::Postgres(store) => {
                store
                    .void_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        reason,
                        now,
                    )
                    .await
            }
        }
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
        match self {
            Self::Memory(store) => {
                store
                    .restore_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        now,
                    )
                    .await
            }
            Self::Postgres(store) => {
                store
                    .restore_case(
                        guild_id,
                        case_number,
                        actor_user_id,
                        expected_version,
                        now,
                    )
                    .await
            }
        }
    }

    pub async fn stats(
        &self,
        guild_id: u64,
        actor_user_id: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<ModeratorStats, CaseError> {
        match self {
            Self::Memory(store) => store.stats(guild_id, actor_user_id, now).await,
            Self::Postgres(store) => store.stats(guild_id, actor_user_id, now).await,
        }
    }
}

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

use super::{
    CaseDetails, CaseError, CaseEvent, CaseEvidence, CaseFilter, CaseNote, CaseStatus, CreateCase,
    ModerationCase, ModeratorStats, UpdateCase, validate_evidence, validate_note,
    validate_void_reason,
};

#[derive(Debug, Clone)]
pub struct PostgresModerationCaseStore {
    pool: PgPool,
}

impl PostgresModerationCaseStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_case(
        &self,
        input: CreateCase,
        now: DateTime<Utc>,
    ) -> Result<ModerationCase, CaseError> {
        input.validate(now)?;
        let mut transaction = self.pool.begin().await?;
        let guild_id = input.guild_id.to_string();
        let case_number: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO moderation_case_counters (guild_id, next_case_number)
            VALUES ($1, 2)
            ON CONFLICT (guild_id)
            DO UPDATE SET next_case_number = moderation_case_counters.next_case_number + 1
            RETURNING next_case_number - 1
            "#,
        )
        .bind(&guild_id)
        .fetch_one(&mut *transaction)
        .await?;
        let case_id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO moderation_cases (
                id, case_number, guild_id, subject_user_id, actor_user_id,
                kind, status, severity, points, reason, source_module,
                visible_to_subject, expires_at, created_at, updated_at, version
            )
            VALUES (
                $1, $2, $3, $4, $5,
                $6, 'active', $7, $8, $9, $10,
                $11, $12, $13, $13, 1
            )
            RETURNING *
            "#,
        )
        .bind(case_id)
        .bind(case_number)
        .bind(&guild_id)
        .bind(input.subject_user_id.to_string())
        .bind(input.actor_user_id.to_string())
        .bind(input.kind.as_str())
        .bind(input.severity.as_str())
        .bind(input.points)
        .bind(&input.reason)
        .bind(&input.source_module)
        .bind(input.visible_to_subject)
        .bind(input.expires_at)
        .bind(now)
        .fetch_one(&mut *transaction)
        .await?;
        insert_event(
            &mut transaction,
            case_id,
            input.actor_user_id,
            "created",
            json!({
                "kind": input.kind,
                "severity": input.severity,
                "points": input.points,
                "visible_to_subject": input.visible_to_subject,
            }),
            now,
        )
        .await?;
        transaction.commit().await?;
        map_case(&row)
    }

    pub async fn get_case(
        &self,
        guild_id: u64,
        case_number: i64,
        now: DateTime<Utc>,
    ) -> Result<CaseDetails, CaseError> {
        self.expire_due(guild_id, now).await?;
        let moderation_case = self.fetch_case(guild_id, case_number).await?;
        let notes = sqlx::query(
            "SELECT * FROM moderation_case_notes WHERE case_id = $1 ORDER BY created_at ASC",
        )
        .bind(moderation_case.id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(map_note)
        .collect::<Result<Vec<_>, _>>()?;
        let evidence = sqlx::query(
            "SELECT * FROM moderation_case_evidence WHERE case_id = $1 ORDER BY created_at ASC",
        )
        .bind(moderation_case.id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(map_evidence)
        .collect::<Result<Vec<_>, _>>()?;
        let events = sqlx::query(
            "SELECT * FROM moderation_case_events WHERE case_id = $1 ORDER BY created_at ASC, id ASC",
        )
        .bind(moderation_case.id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(map_event)
        .collect::<Result<Vec<_>, _>>()?;
        Ok(CaseDetails {
            case: moderation_case,
            notes,
            evidence,
            events,
        })
    }

    pub async fn list_cases(
        &self,
        guild_id: u64,
        filter: CaseFilter,
        now: DateTime<Utc>,
    ) -> Result<Vec<ModerationCase>, CaseError> {
        filter.validate()?;
        self.expire_due(guild_id, now).await?;
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM moderation_cases
            WHERE guild_id = $1
              AND ($2::text IS NULL OR subject_user_id = $2)
              AND ($3::text IS NULL OR actor_user_id = $3)
              AND ($4::text IS NULL OR kind = $4)
              AND ($5::text IS NULL OR status = $5)
              AND (NOT $6 OR visible_to_subject)
            ORDER BY case_number DESC
            LIMIT $7
            "#,
        )
        .bind(guild_id.to_string())
        .bind(filter.subject_user_id.map(|value| value.to_string()))
        .bind(filter.actor_user_id.map(|value| value.to_string()))
        .bind(filter.kind.map(|value| value.as_str().to_owned()))
        .bind(filter.status.map(|value| value.as_str().to_owned()))
        .bind(filter.visible_to_subject_only)
        .bind(i64::from(filter.limit))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_case).collect()
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
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            UPDATE moderation_cases
            SET reason = COALESCE($4, reason),
                severity = COALESCE($5, severity),
                points = COALESCE($6, points),
                visible_to_subject = COALESCE($7, visible_to_subject),
                expires_at = CASE
                    WHEN $8 THEN NULL
                    WHEN $9::timestamptz IS NOT NULL THEN $9
                    ELSE expires_at
                END,
                status = CASE
                    WHEN $8 OR $9::timestamptz IS NOT NULL THEN 'active'
                    ELSE status
                END,
                updated_at = $10,
                version = version + 1
            WHERE guild_id = $1
              AND case_number = $2
              AND version = $3
              AND status <> 'voided'
            RETURNING *
            "#,
        )
        .bind(guild_id.to_string())
        .bind(case_number)
        .bind(expected_version)
        .bind(update.reason)
        .bind(update.severity.map(|value| value.as_str().to_owned()))
        .bind(update.points)
        .bind(update.visible_to_subject)
        .bind(update.clear_expiry)
        .bind(update.expires_at)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                let current = fetch_case_in_transaction(&mut transaction, guild_id, case_number)
                    .await?;
                if current.status == CaseStatus::Voided {
                    return Err(CaseError::Validation(
                        "voided cases must be restored before editing".to_owned(),
                    ));
                }
                return Err(CaseError::VersionConflict {
                    expected: expected_version,
                    actual: current.version,
                });
            }
        };
        let updated = map_case(&row)?;
        insert_event(
            &mut transaction,
            updated.id,
            actor_user_id,
            "updated",
            json!({"version": updated.version}),
            now,
        )
        .await?;
        transaction.commit().await?;
        Ok(updated)
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
        let mut transaction = self.pool.begin().await?;
        let moderation_case =
            fetch_case_in_transaction(&mut transaction, guild_id, case_number).await?;
        let note_id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO moderation_case_notes (
                id, case_id, author_user_id, body, visible_to_subject, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(note_id)
        .bind(moderation_case.id)
        .bind(author_user_id.to_string())
        .bind(&body)
        .bind(visible_to_subject)
        .bind(now)
        .fetch_one(&mut *transaction)
        .await?;
        insert_event(
            &mut transaction,
            moderation_case.id,
            author_user_id,
            "note_added",
            json!({"note_id": note_id, "visible_to_subject": visible_to_subject}),
            now,
        )
        .await?;
        transaction.commit().await?;
        map_note(&row)
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
        let mut transaction = self.pool.begin().await?;
        let moderation_case =
            fetch_case_in_transaction(&mut transaction, guild_id, case_number).await?;
        let evidence_id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO moderation_case_evidence (
                id, case_id, author_user_id, label, value, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(evidence_id)
        .bind(moderation_case.id)
        .bind(author_user_id.to_string())
        .bind(&label)
        .bind(&value)
        .bind(now)
        .fetch_one(&mut *transaction)
        .await?;
        insert_event(
            &mut transaction,
            moderation_case.id,
            author_user_id,
            "evidence_added",
            json!({"evidence_id": evidence_id, "label": label}),
            now,
        )
        .await?;
        transaction.commit().await?;
        map_evidence(&row)
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
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            UPDATE moderation_cases
            SET status = 'voided',
                voided_by_user_id = $4,
                void_reason = $5,
                voided_at = $6,
                updated_at = $6,
                version = version + 1
            WHERE guild_id = $1
              AND case_number = $2
              AND version = $3
              AND status <> 'voided'
            RETURNING *
            "#,
        )
        .bind(guild_id.to_string())
        .bind(case_number)
        .bind(expected_version)
        .bind(actor_user_id.to_string())
        .bind(&reason)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                let current = fetch_case_in_transaction(&mut transaction, guild_id, case_number)
                    .await?;
                if current.status == CaseStatus::Voided {
                    return Err(CaseError::Validation(
                        "moderation case is already voided".to_owned(),
                    ));
                }
                return Err(CaseError::VersionConflict {
                    expected: expected_version,
                    actual: current.version,
                });
            }
        };
        let updated = map_case(&row)?;
        insert_event(
            &mut transaction,
            updated.id,
            actor_user_id,
            "voided",
            json!({"reason": reason, "version": updated.version}),
            now,
        )
        .await?;
        transaction.commit().await?;
        Ok(updated)
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
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            UPDATE moderation_cases
            SET status = CASE
                    WHEN expires_at IS NOT NULL AND expires_at <= $4 THEN 'expired'
                    ELSE 'active'
                END,
                voided_by_user_id = NULL,
                void_reason = NULL,
                voided_at = NULL,
                updated_at = $4,
                version = version + 1
            WHERE guild_id = $1
              AND case_number = $2
              AND version = $3
              AND status = 'voided'
            RETURNING *
            "#,
        )
        .bind(guild_id.to_string())
        .bind(case_number)
        .bind(expected_version)
        .bind(now)
        .fetch_optional(&mut *transaction)
        .await?;
        let row = match row {
            Some(row) => row,
            None => {
                let current = fetch_case_in_transaction(&mut transaction, guild_id, case_number)
                    .await?;
                if current.status != CaseStatus::Voided {
                    return Err(CaseError::Validation(
                        "only voided moderation cases can be restored".to_owned(),
                    ));
                }
                return Err(CaseError::VersionConflict {
                    expected: expected_version,
                    actual: current.version,
                });
            }
        };
        let updated = map_case(&row)?;
        insert_event(
            &mut transaction,
            updated.id,
            actor_user_id,
            "restored",
            json!({"status": updated.status, "version": updated.version}),
            now,
        )
        .await?;
        transaction.commit().await?;
        Ok(updated)
    }

    pub async fn stats(
        &self,
        guild_id: u64,
        actor_user_id: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<ModeratorStats, CaseError> {
        self.expire_due(guild_id, now).await?;
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*)::bigint AS total_cases,
                COUNT(*) FILTER (WHERE status = 'active')::bigint AS active_cases,
                COUNT(*) FILTER (WHERE status = 'expired')::bigint AS expired_cases,
                COUNT(*) FILTER (WHERE status = 'voided')::bigint AS voided_cases,
                COUNT(*) FILTER (WHERE kind = 'warning')::bigint AS warning_cases,
                COUNT(*) FILTER (WHERE kind = 'timeout')::bigint AS timeout_cases,
                COUNT(*) FILTER (WHERE kind = 'kick')::bigint AS kick_cases,
                COUNT(*) FILTER (WHERE kind = 'ban')::bigint AS ban_cases,
                COALESCE(SUM(points), 0)::bigint AS total_points
            FROM moderation_cases
            WHERE guild_id = $1
              AND ($2::text IS NULL OR actor_user_id = $2)
            "#,
        )
        .bind(guild_id.to_string())
        .bind(actor_user_id.map(|value| value.to_string()))
        .fetch_one(&self.pool)
        .await?;
        Ok(ModeratorStats {
            guild_id,
            actor_user_id,
            total_cases: to_u64_count(row.try_get::<i64, _>("total_cases")?)?,
            active_cases: to_u64_count(row.try_get::<i64, _>("active_cases")?)?,
            expired_cases: to_u64_count(row.try_get::<i64, _>("expired_cases")?)?,
            voided_cases: to_u64_count(row.try_get::<i64, _>("voided_cases")?)?,
            warning_cases: to_u64_count(row.try_get::<i64, _>("warning_cases")?)?,
            timeout_cases: to_u64_count(row.try_get::<i64, _>("timeout_cases")?)?,
            kick_cases: to_u64_count(row.try_get::<i64, _>("kick_cases")?)?,
            ban_cases: to_u64_count(row.try_get::<i64, _>("ban_cases")?)?,
            total_points: row.try_get("total_points")?,
        })
    }

    async fn fetch_case(
        &self,
        guild_id: u64,
        case_number: i64,
    ) -> Result<ModerationCase, CaseError> {
        let row = sqlx::query(
            "SELECT * FROM moderation_cases WHERE guild_id = $1 AND case_number = $2",
        )
        .bind(guild_id.to_string())
        .bind(case_number)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(CaseError::NotFound)?;
        map_case(&row)
    }

    async fn expire_due(
        &self,
        guild_id: u64,
        now: DateTime<Utc>,
    ) -> Result<(), CaseError> {
        let mut transaction = self.pool.begin().await?;
        let rows = sqlx::query(
            r#"
            UPDATE moderation_cases
            SET status = 'expired', updated_at = $2, version = version + 1
            WHERE guild_id = $1
              AND status = 'active'
              AND expires_at IS NOT NULL
              AND expires_at <= $2
            RETURNING id, actor_user_id, version
            "#,
        )
        .bind(guild_id.to_string())
        .bind(now)
        .fetch_all(&mut *transaction)
        .await?;
        for row in rows {
            let case_id: Uuid = row.try_get("id")?;
            let actor_user_id = parse_snowflake(row.try_get::<String, _>("actor_user_id")?)?;
            let version: i64 = row.try_get("version")?;
            insert_event(
                &mut transaction,
                case_id,
                actor_user_id,
                "expired",
                json!({"version": version}),
                now,
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

async fn fetch_case_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: u64,
    case_number: i64,
) -> Result<ModerationCase, CaseError> {
    let row = sqlx::query(
        "SELECT * FROM moderation_cases WHERE guild_id = $1 AND case_number = $2 FOR UPDATE",
    )
    .bind(guild_id.to_string())
    .bind(case_number)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(CaseError::NotFound)?;
    map_case(&row)
}

async fn insert_event(
    transaction: &mut Transaction<'_, Postgres>,
    case_id: Uuid,
    actor_user_id: u64,
    event_type: &str,
    payload: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<(), CaseError> {
    sqlx::query(
        r#"
        INSERT INTO moderation_case_events (
            id, case_id, actor_user_id, event_type, payload, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(case_id)
    .bind(actor_user_id.to_string())
    .bind(event_type)
    .bind(payload)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn map_case(row: &PgRow) -> Result<ModerationCase, CaseError> {
    Ok(ModerationCase {
        id: row.try_get("id")?,
        case_number: row.try_get("case_number")?,
        guild_id: parse_snowflake(row.try_get::<String, _>("guild_id")?)?,
        subject_user_id: parse_snowflake(row.try_get::<String, _>("subject_user_id")?)?,
        actor_user_id: parse_snowflake(row.try_get::<String, _>("actor_user_id")?)?,
        kind: row.try_get::<String, _>("kind")?.parse()?,
        status: row.try_get::<String, _>("status")?.parse()?,
        severity: row.try_get::<String, _>("severity")?.parse()?,
        points: row.try_get("points")?,
        reason: row.try_get("reason")?,
        source_module: row.try_get("source_module")?,
        visible_to_subject: row.try_get("visible_to_subject")?,
        expires_at: row.try_get("expires_at")?,
        voided_by_user_id: row
            .try_get::<Option<String>, _>("voided_by_user_id")?
            .map(parse_snowflake)
            .transpose()?,
        void_reason: row.try_get("void_reason")?,
        voided_at: row.try_get("voided_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        version: row.try_get("version")?,
    })
}

fn map_note(row: &PgRow) -> Result<CaseNote, CaseError> {
    Ok(CaseNote {
        id: row.try_get("id")?,
        case_id: row.try_get("case_id")?,
        author_user_id: parse_snowflake(row.try_get::<String, _>("author_user_id")?)?,
        body: row.try_get("body")?,
        visible_to_subject: row.try_get("visible_to_subject")?,
        created_at: row.try_get("created_at")?,
    })
}

fn map_evidence(row: &PgRow) -> Result<CaseEvidence, CaseError> {
    Ok(CaseEvidence {
        id: row.try_get("id")?,
        case_id: row.try_get("case_id")?,
        author_user_id: parse_snowflake(row.try_get::<String, _>("author_user_id")?)?,
        label: row.try_get("label")?,
        value: row.try_get("value")?,
        created_at: row.try_get("created_at")?,
    })
}

fn map_event(row: &PgRow) -> Result<CaseEvent, CaseError> {
    Ok(CaseEvent {
        id: row.try_get("id")?,
        case_id: row.try_get("case_id")?,
        actor_user_id: parse_snowflake(row.try_get::<String, _>("actor_user_id")?)?,
        event_type: row.try_get("event_type")?,
        payload: row.try_get("payload")?,
        created_at: row.try_get("created_at")?,
    })
}

fn parse_snowflake(value: String) -> Result<u64, CaseError> {
    value.parse::<u64>().map_err(|error| {
        CaseError::Validation(format!("invalid Discord snowflake stored in database: {error}"))
    })
}

fn to_u64_count(value: i64) -> Result<u64, CaseError> {
    u64::try_from(value)
        .map_err(|_| CaseError::Validation("database returned a negative count".to_owned()))
}

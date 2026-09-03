use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use uuid::Uuid;

use super::model::{
    NewTicketReservation, NewTicketTranscript, ParticipantMutationOutcome, ReserveTicketOutcome,
    STATUS_CLAIMED, STATUS_CLOSED, STATUS_OPEN, STATUS_PROVISIONING, TicketEvent, TicketMutation,
    TicketMutationOutcome, TicketParticipant, TicketRecord, TicketTranscript,
};

#[derive(Clone)]
pub struct PostgresTicketStore {
    pool: PgPool,
}

impl PostgresTicketStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn reserve_ticket(
        &self,
        reservation: NewTicketReservation,
        max_open_per_user: u8,
    ) -> anyhow::Result<ReserveTicketOutcome> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1), hashtext($2))")
            .bind(&reservation.guild_id)
            .bind(&reservation.creator_user_id)
            .fetch_optional(&mut *transaction)
            .await?;

        sqlx::query(
            r#"
            WITH expired AS (
                UPDATE tickets
                SET status = 'failed',
                    provisioning_error = 'ticket provisioning lease expired',
                    closed_at = NOW(),
                    updated_at = NOW(),
                    version = version + 1
                WHERE guild_id = $1
                  AND status = 'provisioning'
                  AND created_at < NOW() - INTERVAL '10 minutes'
                RETURNING id, guild_id
            )
            INSERT INTO ticket_events (ticket_id, guild_id, actor_user_id, event_type, data)
            SELECT id, guild_id, NULL, 'ticket_provisioning_expired', '{}'::jsonb
            FROM expired
            "#,
        )
        .bind(&reservation.guild_id)
        .execute(&mut *transaction)
        .await?;

        let active_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM tickets
            WHERE guild_id = $1
              AND creator_user_id = $2
              AND status IN ('provisioning', 'open', 'claimed')
            "#,
        )
        .bind(&reservation.guild_id)
        .bind(&reservation.creator_user_id)
        .fetch_one(&mut *transaction)
        .await?;

        if active_count >= i64::from(max_open_per_user) {
            transaction.commit().await?;
            return Ok(ReserveTicketOutcome::LimitReached {
                limit: max_open_per_user,
                active_count: u8::try_from(active_count).unwrap_or(u8::MAX),
            });
        }

        let ticket_id = Uuid::new_v4().to_string();
        let ticket = sqlx::query_as::<_, TicketRecord>(
            r#"
            INSERT INTO tickets (
                id,
                guild_id,
                creator_user_id,
                subject,
                description,
                queue,
                priority,
                status,
                last_activity_at
            )
            VALUES ($1::uuid, $2, $3, $4, $5, $6, 'normal', 'provisioning', NOW())
            RETURNING
                id::text AS id,
                number,
                guild_id,
                creator_user_id,
                channel_id,
                channel_name,
                subject,
                description,
                queue,
                priority,
                status,
                claimed_by_user_id,
                first_response_at,
                last_activity_at,
                close_reason,
                closed_by_user_id,
                provisioning_error,
                version,
                created_at,
                updated_at,
                closed_at
            "#,
        )
        .bind(&ticket_id)
        .bind(&reservation.guild_id)
        .bind(&reservation.creator_user_id)
        .bind(reservation.subject.trim())
        .bind(reservation.description.trim())
        .bind(reservation.queue.trim())
        .fetch_one(&mut *transaction)
        .await?;

        insert_event(
            &mut transaction,
            &ticket,
            Some(&reservation.creator_user_id),
            "ticket_reserved",
            json!({
                "number": ticket.number,
                "queue": ticket.queue,
                "subject": ticket.subject,
            }),
        )
        .await?;
        transaction.commit().await?;
        Ok(ReserveTicketOutcome::Reserved(Box::new(ticket)))
    }

    pub async fn activate_ticket(
        &self,
        ticket_id: &str,
        channel_id: &str,
        channel_name: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            None,
            "ticket_opened",
            json!({
                "channel_id": channel_id,
                "channel_name": channel_name,
            }),
            |ticket| {
                if ticket.status != STATUS_PROVISIONING {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.activate(channel_id.to_owned(), channel_name.to_owned(), Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn fail_provisioning(
        &self,
        ticket_id: &str,
        error: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            None,
            "ticket_provisioning_failed",
            json!({ "error": error.chars().take(1_000).collect::<String>() }),
            |ticket| {
                if ticket.status != STATUS_PROVISIONING {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.fail_provisioning(error, Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn get_by_channel(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> anyhow::Result<Option<TicketRecord>> {
        Ok(sqlx::query_as::<_, TicketRecord>(
            r#"
            SELECT
                id::text AS id,
                number,
                guild_id,
                creator_user_id,
                channel_id,
                channel_name,
                subject,
                description,
                queue,
                priority,
                status,
                claimed_by_user_id,
                first_response_at,
                last_activity_at,
                close_reason,
                closed_by_user_id,
                provisioning_error,
                version,
                created_at,
                updated_at,
                closed_at
            FROM tickets
            WHERE guild_id = $1 AND channel_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_tickets(
        &self,
        guild_id: &str,
        creator_user_id: &str,
        include_all: bool,
        limit: u8,
    ) -> anyhow::Result<Vec<TicketRecord>> {
        if include_all {
            Ok(sqlx::query_as::<_, TicketRecord>(
                r#"
                SELECT
                    id::text AS id,
                    number,
                    guild_id,
                    creator_user_id,
                    channel_id,
                    channel_name,
                    subject,
                    description,
                    queue,
                    priority,
                    status,
                    claimed_by_user_id,
                    first_response_at,
                    last_activity_at,
                    close_reason,
                    closed_by_user_id,
                    provisioning_error,
                    version,
                    created_at,
                    updated_at,
                    closed_at
                FROM tickets
                WHERE guild_id = $1
                  AND status IN ('provisioning', 'open', 'claimed')
                ORDER BY
                    CASE priority
                        WHEN 'urgent' THEN 4
                        WHEN 'high' THEN 3
                        WHEN 'normal' THEN 2
                        WHEN 'low' THEN 1
                        ELSE 0
                    END DESC,
                    created_at ASC,
                    number ASC
                LIMIT $2
                "#,
            )
            .bind(guild_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
        } else {
            Ok(sqlx::query_as::<_, TicketRecord>(
                r#"
                SELECT
                    id::text AS id,
                    number,
                    guild_id,
                    creator_user_id,
                    channel_id,
                    channel_name,
                    subject,
                    description,
                    queue,
                    priority,
                    status,
                    claimed_by_user_id,
                    first_response_at,
                    last_activity_at,
                    close_reason,
                    closed_by_user_id,
                    provisioning_error,
                    version,
                    created_at,
                    updated_at,
                    closed_at
                FROM tickets
                WHERE guild_id = $1
                  AND creator_user_id = $2
                  AND status IN ('provisioning', 'open', 'claimed')
                ORDER BY
                    CASE priority
                        WHEN 'urgent' THEN 4
                        WHEN 'high' THEN 3
                        WHEN 'normal' THEN 2
                        WHEN 'low' THEN 1
                        ELSE 0
                    END DESC,
                    created_at ASC,
                    number ASC
                LIMIT $3
                "#,
            )
            .bind(guild_id)
            .bind(creator_user_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
        }
    }

    pub async fn claim_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::Claim.event_name(),
            json!({ "claimed_by_user_id": actor_user_id }),
            |ticket| match ticket.status.as_str() {
                STATUS_OPEN => {
                    ticket.claim(actor_user_id, Utc::now());
                    Ok(())
                }
                STATUS_CLAIMED if ticket.claimed_by_user_id.as_deref() == Some(actor_user_id) => {
                    Err(TicketMutationOutcome::Conflict {
                        message: "ticket is already claimed by this user".to_owned(),
                    })
                }
                STATUS_CLAIMED => Err(TicketMutationOutcome::Conflict {
                    message: "ticket is already claimed by another staff member".to_owned(),
                }),
                _ => Err(TicketMutationOutcome::InvalidState {
                    current_status: ticket.status.clone(),
                }),
            },
        )
        .await
    }

    pub async fn unclaim_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        allow_any: bool,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::Unclaim.event_name(),
            json!({}),
            |ticket| {
                if ticket.status != STATUS_CLAIMED {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                if !allow_any && ticket.claimed_by_user_id.as_deref() != Some(actor_user_id) {
                    return Err(TicketMutationOutcome::Forbidden);
                }
                ticket.unclaim(Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn close_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        reason: Option<&str>,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::Close.event_name(),
            json!({ "reason": reason }),
            |ticket| {
                if !ticket.is_open() {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.close(actor_user_id, reason, Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn reopen_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::Reopen.event_name(),
            json!({}),
            |ticket| {
                if ticket.status != STATUS_CLOSED {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.reopen(Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn rename_ticket(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        channel_name: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::Rename.event_name(),
            json!({ "channel_name": channel_name }),
            |ticket| {
                if !ticket.is_open() {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.rename(channel_name.to_owned(), Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn set_priority(
        &self,
        ticket_id: &str,
        actor_user_id: &str,
        priority: &str,
    ) -> anyhow::Result<TicketMutationOutcome> {
        self.mutate_ticket(
            ticket_id,
            Some(actor_user_id),
            TicketMutation::SetPriority.event_name(),
            json!({ "priority": priority }),
            |ticket| {
                if !ticket.is_open() {
                    return Err(TicketMutationOutcome::InvalidState {
                        current_status: ticket.status.clone(),
                    });
                }
                ticket.set_priority(priority, Utc::now());
                Ok(())
            },
        )
        .await
    }

    pub async fn add_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> anyhow::Result<ParticipantMutationOutcome> {
        let mut transaction = self.pool.begin().await?;
        let Some(ticket) = fetch_ticket_for_update(&mut transaction, ticket_id).await? else {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::NotFound);
        };
        if !ticket.is_open() {
            let current_status = ticket.status;
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::InvalidState { current_status });
        }
        if ticket.creator_user_id == user_id {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::CreatorProtected);
        }

        let participant = sqlx::query_as::<_, TicketParticipant>(
            r#"
            INSERT INTO ticket_participants (ticket_id, user_id, added_by_user_id)
            VALUES ($1::uuid, $2, $3)
            ON CONFLICT (ticket_id, user_id) DO NOTHING
            RETURNING
                ticket_id::text AS ticket_id,
                user_id,
                added_by_user_id,
                created_at
            "#,
        )
        .bind(ticket_id)
        .bind(user_id)
        .bind(actor_user_id)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some(participant) = participant else {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::AlreadyPresent);
        };
        insert_event(
            &mut transaction,
            &ticket,
            Some(actor_user_id),
            "ticket_participant_added",
            json!({ "user_id": user_id }),
        )
        .await?;
        transaction.commit().await?;
        Ok(ParticipantMutationOutcome::Added(participant))
    }

    pub async fn remove_participant(
        &self,
        ticket_id: &str,
        user_id: &str,
        actor_user_id: &str,
    ) -> anyhow::Result<ParticipantMutationOutcome> {
        let mut transaction = self.pool.begin().await?;
        let Some(ticket) = fetch_ticket_for_update(&mut transaction, ticket_id).await? else {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::NotFound);
        };
        if !ticket.is_open() {
            let current_status = ticket.status;
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::InvalidState { current_status });
        }
        if ticket.creator_user_id == user_id {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::CreatorProtected);
        }

        let result = sqlx::query(
            "DELETE FROM ticket_participants WHERE ticket_id = $1::uuid AND user_id = $2",
        )
        .bind(ticket_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.commit().await?;
            return Ok(ParticipantMutationOutcome::NotFound);
        }

        insert_event(
            &mut transaction,
            &ticket,
            Some(actor_user_id),
            "ticket_participant_removed",
            json!({ "user_id": user_id }),
        )
        .await?;
        transaction.commit().await?;
        Ok(ParticipantMutationOutcome::Removed)
    }

    pub async fn list_participants(
        &self,
        ticket_id: &str,
    ) -> anyhow::Result<Vec<TicketParticipant>> {
        Ok(sqlx::query_as::<_, TicketParticipant>(
            r#"
            SELECT
                ticket_id::text AS ticket_id,
                user_id,
                added_by_user_id,
                created_at
            FROM ticket_participants
            WHERE ticket_id = $1::uuid
            ORDER BY created_at ASC, user_id ASC
            "#,
        )
        .bind(ticket_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn store_transcript(
        &self,
        transcript: NewTicketTranscript,
    ) -> anyhow::Result<TicketTranscript> {
        let mut transaction = self.pool.begin().await?;
        let Some(ticket) = fetch_ticket_for_update(&mut transaction, &transcript.ticket_id).await?
        else {
            anyhow::bail!("ticket no longer exists while storing its transcript");
        };
        let transcript_id = Uuid::new_v4().to_string();
        let stored = sqlx::query_as::<_, TicketTranscript>(
            r#"
            INSERT INTO ticket_transcripts (
                id,
                ticket_id,
                guild_id,
                channel_id,
                generated_by_user_id,
                message_count,
                content
            )
            VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
            RETURNING
                id::text AS id,
                ticket_id::text AS ticket_id,
                guild_id,
                channel_id,
                generated_by_user_id,
                message_count,
                content,
                created_at
            "#,
        )
        .bind(&transcript_id)
        .bind(&transcript.ticket_id)
        .bind(&transcript.guild_id)
        .bind(&transcript.channel_id)
        .bind(&transcript.generated_by_user_id)
        .bind(transcript.message_count)
        .bind(&transcript.content)
        .fetch_one(&mut *transaction)
        .await?;
        insert_event(
            &mut transaction,
            &ticket,
            Some(&transcript.generated_by_user_id),
            "ticket_transcript_created",
            json!({
                "transcript_id": stored.id,
                "message_count": stored.message_count,
            }),
        )
        .await?;
        transaction.commit().await?;
        Ok(stored)
    }

    pub async fn list_events(
        &self,
        ticket_id: &str,
        limit: u16,
    ) -> anyhow::Result<Vec<TicketEvent>> {
        Ok(sqlx::query_as::<_, TicketEvent>(
            r#"
            SELECT
                id,
                ticket_id::text AS ticket_id,
                guild_id,
                actor_user_id,
                event_type,
                data,
                created_at
            FROM ticket_events
            WHERE ticket_id = $1::uuid
            ORDER BY id DESC
            LIMIT $2
            "#,
        )
        .bind(ticket_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?)
    }

    async fn mutate_ticket<F>(
        &self,
        ticket_id: &str,
        actor_user_id: Option<&str>,
        event_type: &str,
        data: Value,
        mutate: F,
    ) -> anyhow::Result<TicketMutationOutcome>
    where
        F: FnOnce(&mut TicketRecord) -> Result<(), TicketMutationOutcome>,
    {
        if Uuid::parse_str(ticket_id).is_err() {
            return Ok(TicketMutationOutcome::NotFound);
        }

        let mut transaction = self.pool.begin().await?;
        let Some(mut ticket) = fetch_ticket_for_update(&mut transaction, ticket_id).await? else {
            transaction.commit().await?;
            return Ok(TicketMutationOutcome::NotFound);
        };

        if let Err(outcome) = mutate(&mut ticket) {
            transaction.commit().await?;
            return Ok(outcome);
        }
        write_ticket(&mut transaction, &ticket).await?;
        insert_event(
            &mut transaction,
            &ticket,
            actor_user_id,
            event_type,
            data,
        )
        .await?;
        transaction.commit().await?;
        Ok(TicketMutationOutcome::Updated(Box::new(ticket)))
    }
}

async fn fetch_ticket_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    ticket_id: &str,
) -> Result<Option<TicketRecord>, sqlx::Error> {
    sqlx::query_as::<_, TicketRecord>(
        r#"
        SELECT
            id::text AS id,
            number,
            guild_id,
            creator_user_id,
            channel_id,
            channel_name,
            subject,
            description,
            queue,
            priority,
            status,
            claimed_by_user_id,
            first_response_at,
            last_activity_at,
            close_reason,
            closed_by_user_id,
            provisioning_error,
            version,
            created_at,
            updated_at,
            closed_at
        FROM tickets
        WHERE id = $1::uuid
        FOR UPDATE
        "#,
    )
    .bind(ticket_id)
    .fetch_optional(&mut **transaction)
    .await
}

async fn write_ticket(
    transaction: &mut Transaction<'_, Postgres>,
    ticket: &TicketRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE tickets
        SET channel_id = $2,
            channel_name = $3,
            priority = $4,
            status = $5,
            claimed_by_user_id = $6,
            first_response_at = $7,
            last_activity_at = $8,
            close_reason = $9,
            closed_by_user_id = $10,
            provisioning_error = $11,
            version = $12,
            updated_at = $13,
            closed_at = $14
        WHERE id = $1::uuid
        "#,
    )
    .bind(&ticket.id)
    .bind(&ticket.channel_id)
    .bind(&ticket.channel_name)
    .bind(&ticket.priority)
    .bind(&ticket.status)
    .bind(&ticket.claimed_by_user_id)
    .bind(ticket.first_response_at)
    .bind(ticket.last_activity_at)
    .bind(&ticket.close_reason)
    .bind(&ticket.closed_by_user_id)
    .bind(&ticket.provisioning_error)
    .bind(ticket.version)
    .bind(ticket.updated_at)
    .bind(ticket.closed_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_event(
    transaction: &mut Transaction<'_, Postgres>,
    ticket: &TicketRecord,
    actor_user_id: Option<&str>,
    event_type: &str,
    data: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO ticket_events (ticket_id, guild_id, actor_user_id, event_type, data)
        VALUES ($1::uuid, $2, $3, $4, $5)
        "#,
    )
    .bind(&ticket.id)
    .bind(&ticket.guild_id)
    .bind(actor_user_id)
    .bind(event_type)
    .bind(data)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

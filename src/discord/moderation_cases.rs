use anyhow::{Result, anyhow};
use chrono::{Duration, Utc};
use serenity::{
    all::{ChannelId, GuildId, Permissions, UserId},
    builder::EditMember,
    client::Context,
};

use crate::{
    AppState,
    lua::ModerationCaseOperation,
    moderation_cases::{CaseFilter, CaseKind, CaseSeverity, CaseStatus, CreateCase, UpdateCase},
};

#[derive(Clone, Copy)]
pub struct ModerationExecutionContext {
    pub guild_id: Option<GuildId>,
    pub channel_id: Option<ChannelId>,
    pub actor_id: Option<UserId>,
    pub actor_permissions: Permissions,
    pub app_permissions: Permissions,
    pub command_context: bool,
}

impl ModerationExecutionContext {
    #[must_use]
    pub const fn new(
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        actor_id: Option<UserId>,
        actor_permissions: Permissions,
        app_permissions: Permissions,
        command_context: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            actor_id,
            actor_permissions,
            app_permissions,
            command_context,
        }
    }
}

pub fn is_moderator(permissions: Permissions) -> bool {
    permissions.contains(Permissions::ADMINISTRATOR)
        || permissions.contains(Permissions::MANAGE_GUILD)
        || permissions.contains(Permissions::MODERATE_MEMBERS)
        || permissions.contains(Permissions::BAN_MEMBERS)
        || permissions.contains(Permissions::KICK_MEMBERS)
        || permissions.contains(Permissions::MANAGE_MESSAGES)
}

pub async fn execute(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    execution: ModerationExecutionContext,
    operation: &ModerationCaseOperation,
) -> Result<Option<String>> {
    let guild_id = execution
        .guild_id
        .ok_or_else(|| anyhow!("system spraw moderacyjnych działa wyłącznie na serwerze"))?;
    let actor_id = execution
        .actor_id
        .ok_or_else(|| anyhow!("ta operacja wymaga kontekstu użytkownika"))?;
    let now = Utc::now();
    let is_mod = is_moderator(execution.actor_permissions);

    match operation {
        ModerationCaseOperation::Create {
            subject_user_id,
            kind,
            reason,
            severity,
            points,
            visible_to_subject,
            duration_seconds,
        } => {
            if !is_mod {
                return Err(anyhow!("brak uprawnień moderatorskich do tworzenia spraw"));
            }

            let subject_u64 = subject_user_id
                .parse::<u64>()
                .map_err(|_| anyhow!("niepoprawne ID użytkownika"))?;
            let target_user_id = UserId::new(subject_u64);

            let case_kind = match kind.as_str() {
                "warning" => CaseKind::Warning,
                "staff_note" => CaseKind::StaffNote,
                "timeout" => CaseKind::Timeout,
                "kick" => CaseKind::Kick,
                "ban" => CaseKind::Ban,
                "unban" => CaseKind::Unban,
                "softban" => CaseKind::Softban,
                "automod" => CaseKind::Automod,
                _ => CaseKind::Other,
            };

            if matches!(
                case_kind,
                CaseKind::Ban | CaseKind::Unban | CaseKind::Softban
            ) && !execution
                .actor_permissions
                .contains(Permissions::BAN_MEMBERS)
                && !execution
                    .actor_permissions
                    .contains(Permissions::ADMINISTRATOR)
            {
                return Err(anyhow!(
                    "brak uprawnienia BAN_MEMBERS do wykonania tej akcji"
                ));
            }
            if case_kind == CaseKind::Kick
                && !execution
                    .actor_permissions
                    .contains(Permissions::KICK_MEMBERS)
                && !execution
                    .actor_permissions
                    .contains(Permissions::ADMINISTRATOR)
            {
                return Err(anyhow!(
                    "brak uprawnienia KICK_MEMBERS do wykonania tej akcji"
                ));
            }
            if case_kind == CaseKind::Timeout
                && !execution
                    .actor_permissions
                    .contains(Permissions::MODERATE_MEMBERS)
                && !execution
                    .actor_permissions
                    .contains(Permissions::ADMINISTRATOR)
            {
                return Err(anyhow!(
                    "brak uprawnienia MODERATE_MEMBERS do wykonania tej akcji"
                ));
            }

            if execution.command_context && case_kind != CaseKind::Unban {
                crate::discord::executor::ensure_member_hierarchy(
                    ctx,
                    guild_id,
                    execution.actor_id,
                    target_user_id,
                )
                .await?;
            }

            let case_severity = match severity.as_str() {
                "info" => CaseSeverity::Info,
                "low" => CaseSeverity::Low,
                "medium" => CaseSeverity::Medium,
                "high" => CaseSeverity::High,
                "critical" => CaseSeverity::Critical,
                _ => CaseSeverity::Medium,
            };

            let expires_at = duration_seconds.map(|secs| now + Duration::seconds(secs as i64));

            // Execute underlying Discord action if applicable
            if case_kind == CaseKind::Timeout {
                if let Some(secs) = duration_seconds {
                    let until = (now + Duration::seconds(*secs as i64)).to_rfc3339();
                    let _ = guild_id
                        .edit_member(
                            &ctx.http,
                            target_user_id,
                            EditMember::new().disable_communication_until(until),
                        )
                        .await;
                }
            } else if case_kind == CaseKind::Kick {
                let _ = guild_id
                    .kick_with_reason(&ctx.http, target_user_id, reason)
                    .await;
            } else if case_kind == CaseKind::Ban {
                let _ = guild_id
                    .ban_with_reason(&ctx.http, target_user_id, 0, reason)
                    .await;
            } else if case_kind == CaseKind::Softban {
                let _ = guild_id
                    .ban_with_reason(&ctx.http, target_user_id, 1, reason)
                    .await;
                let _ = guild_id.unban(&ctx.http, target_user_id).await;
            } else if case_kind == CaseKind::Unban {
                let _ = guild_id.unban(&ctx.http, target_user_id).await;
            }

            let created = state
                .moderation_cases
                .create_case(
                    CreateCase {
                        guild_id: guild_id.get(),
                        subject_user_id: subject_u64,
                        actor_user_id: actor_id.get(),
                        kind: case_kind,
                        severity: case_severity,
                        points: *points,
                        reason: reason.clone(),
                        source_module: module_id.to_owned(),
                        visible_to_subject: *visible_to_subject,
                        expires_at,
                    },
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd zapisu sprawy: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_created",
                    serde_json::json!({
                        "case_number": created.case_number,
                        "subject_user_id": subject_user_id,
                        "kind": kind,
                        "severity": severity,
                        "points": points,
                        "reason": reason,
                    }),
                )
                .await;

            Ok(Some(format!(
                "⚖️ Zarejestrowano sprawę moderacyjną **#{case_number}** dla <@{subject_user_id}> [{kind} | {severity} | punkty: {points}]: {reason}",
                case_number = created.case_number,
            )))
        }

        ModerationCaseOperation::View { case_number } => {
            let details = state
                .moderation_cases
                .get_case(guild_id.get(), *case_number, now)
                .await
                .map_err(|err| anyhow!("sprawa #{case_number} nie została znaleziona: {err}"))?;

            if !is_mod
                && (details.case.subject_user_id != actor_id.get()
                    || !details.case.visible_to_subject)
            {
                return Err(anyhow!(
                    "brak uprawnień do przeglądania sprawy #{case_number}"
                ));
            }

            let c = &details.case;
            let status_label = match c.status {
                CaseStatus::Active => "Aktywna",
                CaseStatus::Expired => "Wygasła",
                CaseStatus::Voided => "Unieważniona",
            };

            let expiry_info = match c.expires_at {
                Some(exp) => format!("Wygasa: {}", exp.to_rfc3339()),
                None => "Wygasa: Nigdy".to_owned(),
            };

            let mut message = format!(
                "📋 **Sprawa #{case_number}** [{kind:?} | {severity:?}] — Status: **{status}**\n• Cel: <@{subject}> | Moderator: <@{actor}>\n• Punkty: {points} | {expiry_info}\n• Powód: {reason}\n• Utworzono: {created}",
                case_number = c.case_number,
                kind = c.kind,
                severity = c.severity,
                status = status_label,
                subject = c.subject_user_id,
                actor = c.actor_user_id,
                points = c.points,
                reason = c.reason,
                created = c.created_at.to_rfc3339(),
            );

            if let Some(void_reason) = &c.void_reason {
                message.push_str(&format!(
                    "\n• ⚠️ Unieważniono przez <@{}>: {}",
                    c.voided_by_user_id.unwrap_or(0),
                    void_reason
                ));
            }

            let visible_notes: Vec<_> = details
                .notes
                .iter()
                .filter(|n| is_mod || n.visible_to_subject)
                .collect();
            if !visible_notes.is_empty() {
                message.push_str("\n\n**Notatki:**");
                for note in visible_notes {
                    message.push_str(&format!(
                        "\n- <@{}> ({}): {}",
                        note.author_user_id,
                        note.created_at.to_rfc3339(),
                        note.body
                    ));
                }
            }

            if !details.evidence.is_empty() && is_mod {
                message.push_str("\n\n**Dowody:**");
                for ev in &details.evidence {
                    message.push_str(&format!("\n- [{}] {}", ev.label, ev.value));
                }
            }

            Ok(Some(message))
        }

        ModerationCaseOperation::List {
            subject_user_id,
            limit,
            visible_to_subject_only,
        } => {
            let filter_subject = if !is_mod {
                Some(actor_id.get())
            } else {
                subject_user_id
                    .as_deref()
                    .and_then(|id| id.parse::<u64>().ok())
            };

            let visible_only = if !is_mod {
                true
            } else {
                *visible_to_subject_only
            };

            let cases = state
                .moderation_cases
                .list_cases(
                    guild_id.get(),
                    CaseFilter {
                        subject_user_id: filter_subject,
                        actor_user_id: None,
                        kind: None,
                        status: None,
                        visible_to_subject_only: visible_only,
                        limit: *limit as u16,
                    },
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd pobierania listy spraw: {err}"))?;

            if cases.is_empty() {
                return Ok(Some(
                    "Brak zarejestrowanych spraw moderacyjnych.".to_owned(),
                ));
            }

            let mut message = format!("📋 **Sprawy moderacyjne ({})**:\n", cases.len());
            for c in cases {
                message.push_str(&format!(
                    "• **#{}** [{:?}] dla <@{}>: {} (Status: {:?})\n",
                    c.case_number, c.kind, c.subject_user_id, c.reason, c.status
                ));
            }

            Ok(Some(message))
        }

        ModerationCaseOperation::AddNote {
            case_number,
            body,
            visible_to_subject,
        } => {
            if !is_mod {
                return Err(anyhow!(
                    "brak uprawnień moderatorskich do dodawania notatek"
                ));
            }

            state
                .moderation_cases
                .add_note(
                    guild_id.get(),
                    *case_number,
                    actor_id.get(),
                    body.clone(),
                    *visible_to_subject,
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd dodawania notatki: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_note_added",
                    serde_json::json!({
                        "case_number": case_number,
                        "body": body,
                        "visible_to_subject": visible_to_subject,
                    }),
                )
                .await;

            Ok(Some(format!("Dodano notatkę do sprawy #{case_number}.")))
        }

        ModerationCaseOperation::AddEvidence {
            case_number,
            label,
            value,
        } => {
            if !is_mod {
                return Err(anyhow!(
                    "brak uprawnień moderatorskich do dodawania dowodów"
                ));
            }

            state
                .moderation_cases
                .add_evidence(
                    guild_id.get(),
                    *case_number,
                    actor_id.get(),
                    label.clone(),
                    value.clone(),
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd dodawania dowodu: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_evidence_added",
                    serde_json::json!({
                        "case_number": case_number,
                        "label": label,
                        "value": value,
                    }),
                )
                .await;

            Ok(Some(format!(
                "Dodano dowód [{label}] do sprawy #{case_number}."
            )))
        }

        ModerationCaseOperation::Void {
            case_number,
            expected_version,
            reason,
        } => {
            if !is_mod {
                return Err(anyhow!(
                    "brak uprawnień moderatorskich do unieważniania spraw"
                ));
            }

            let version = if *expected_version <= 0 {
                let current = state
                    .moderation_cases
                    .get_case(guild_id.get(), *case_number, now)
                    .await
                    .map_err(|err| anyhow!("nie znaleziono sprawy #{case_number}: {err}"))?;
                current.case.version
            } else {
                *expected_version
            };

            state
                .moderation_cases
                .void_case(
                    guild_id.get(),
                    *case_number,
                    actor_id.get(),
                    version,
                    reason.clone(),
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd unieważniania sprawy: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_voided",
                    serde_json::json!({
                        "case_number": case_number,
                        "reason": reason,
                    }),
                )
                .await;

            Ok(Some(format!(
                "Sprawa #{case_number} została unieważniona. Powód: {reason}"
            )))
        }

        ModerationCaseOperation::Edit {
            case_number,
            expected_version,
            reason,
            severity,
            points,
            visible_to_subject,
            duration_seconds,
        } => {
            if !is_mod {
                return Err(anyhow!("brak uprawnień moderatorskich do edycji spraw"));
            }

            let version = if *expected_version <= 0 {
                let current = state
                    .moderation_cases
                    .get_case(guild_id.get(), *case_number, now)
                    .await
                    .map_err(|err| anyhow!("nie znaleziono sprawy #{case_number}: {err}"))?;
                current.case.version
            } else {
                *expected_version
            };

            let sev = severity.as_deref().map(|s| match s {
                "info" => CaseSeverity::Info,
                "low" => CaseSeverity::Low,
                "medium" => CaseSeverity::Medium,
                "high" => CaseSeverity::High,
                "critical" => CaseSeverity::Critical,
                _ => CaseSeverity::Medium,
            });

            let expires_at = duration_seconds.map(|secs| now + Duration::seconds(secs as i64));

            let update = UpdateCase {
                reason: reason.clone(),
                severity: sev,
                points: *points,
                visible_to_subject: *visible_to_subject,
                expires_at,
                clear_expiry: false,
            };

            let updated = state
                .moderation_cases
                .update_case(
                    guild_id.get(),
                    *case_number,
                    actor_id.get(),
                    version,
                    update,
                    now,
                )
                .await
                .map_err(|err| anyhow!("błąd aktualizacji sprawy: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_edited",
                    serde_json::json!({
                        "case_number": case_number,
                        "version": updated.version,
                        "reason": reason,
                        "severity": severity,
                        "points": points,
                    }),
                )
                .await;

            Ok(Some(format!(
                "✏️ Zaktualizowano sprawę moderacyjną **#{case_number}** (wersja {version}).",
                version = updated.version
            )))
        }

        ModerationCaseOperation::Restore {
            case_number,
            expected_version,
        } => {
            if !is_mod {
                return Err(anyhow!(
                    "brak uprawnień moderatorskich do przywracania spraw"
                ));
            }

            let version = if *expected_version <= 0 {
                let current = state
                    .moderation_cases
                    .get_case(guild_id.get(), *case_number, now)
                    .await
                    .map_err(|err| anyhow!("nie znaleziono sprawy #{case_number}: {err}"))?;
                current.case.version
            } else {
                *expected_version
            };

            let restored = state
                .moderation_cases
                .restore_case(guild_id.get(), *case_number, actor_id.get(), version, now)
                .await
                .map_err(|err| anyhow!("błąd przywracania sprawy: {err}"))?;

            let _ = state
                .storage
                .record_audit(
                    Some(&guild_id.get().to_string()),
                    Some(&actor_id.get().to_string()),
                    module_id,
                    "case_restored",
                    serde_json::json!({
                        "case_number": case_number,
                    }),
                )
                .await;

            Ok(Some(format!(
                "Sprawa #{case_number} została przywrócona (Status: {:?}).",
                restored.status
            )))
        }

        ModerationCaseOperation::Stats { moderator_user_id } => {
            if !is_mod {
                return Err(anyhow!(
                    "brak uprawnień moderatorskich do przeglądania statystyk"
                ));
            }

            let mod_id = moderator_user_id
                .as_deref()
                .and_then(|id| id.parse::<u64>().ok());

            let stats = state
                .moderation_cases
                .stats(guild_id.get(), mod_id, now)
                .await
                .map_err(|err| anyhow!("błąd pobierania statystyk: {err}"))?;

            Ok(Some(format!(
                "📊 **Statystyki moderacji**:\n• Wszystkich spraw: {}\n• Aktywnych: {}\n• Wygasłych: {}\n• Unieważnionych: {}",
                stats.total_cases, stats.active_cases, stats.expired_cases, stats.voided_cases
            )))
        }
    }
}

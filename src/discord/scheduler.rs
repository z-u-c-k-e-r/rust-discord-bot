use anyhow::{Context as _, Result, anyhow};
use serenity::{
    all::{Channel, ChannelId, GuildId, Permissions, UserId},
    client::Context,
};

use crate::{
    AppState,
    lua::{SchedulerOperation, SchedulerScope},
    scheduler::{
        CreateJobOutcome, JobMutation, JobMutationOutcome, NewScheduledJob, ScheduledJob,
        time::{parse_repeat_interval, parse_schedule_time},
    },
};

#[derive(Clone, Copy)]
pub(super) struct SchedulerExecutionContext {
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    command_context: bool,
}

impl SchedulerExecutionContext {
    pub(super) const fn new(
        guild_id: Option<GuildId>,
        channel_id: Option<ChannelId>,
        actor_id: Option<UserId>,
        actor_permissions: Permissions,
        command_context: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            actor_id,
            actor_permissions,
            command_context,
        }
    }
}

pub async fn execute(
    ctx: &Context,
    state: &AppState,
    module_id: &str,
    execution: SchedulerExecutionContext,
    operation: &SchedulerOperation,
) -> Result<Option<String>> {
    if module_id != "scheduler" {
        return Err(anyhow!(
            "operacje harmonogramu są zarezerwowane dla zaufanego modułu scheduler"
        ));
    }
    if !execution.command_context {
        return Err(anyhow!(
            "zadania harmonogramu mogą być tworzone i zmieniane tylko przez komendy użytkowników"
        ));
    }

    let guild_id = execution
        .guild_id
        .ok_or_else(|| anyhow!("harmonogram działa wyłącznie na serwerze"))?;
    let actor_id = execution
        .actor_id
        .ok_or_else(|| anyhow!("operacja harmonogramu wymaga użytkownika"))?;
    let can_manage_guild = has_manage_guild(execution.actor_permissions);
    if operation.requires_manage_guild() && !can_manage_guild {
        return Err(anyhow!(
            "ta operacja harmonogramu wymaga uprawnienia Zarządzanie serwerem"
        ));
    }

    let guild_key = guild_id.get().to_string();
    let actor_key = actor_id.get().to_string();

    match operation {
        SchedulerOperation::Create {
            when,
            content,
            channel_id,
            repeat,
            repeat_count,
            mention_creator,
            max_jobs,
            minimum_delay_seconds,
            maximum_delay_seconds,
        } => {
            let now = chrono::Utc::now();
            let run_at = parse_schedule_time(
                when,
                now,
                *minimum_delay_seconds,
                *maximum_delay_seconds,
            )
            .map_err(|error| anyhow!("nieprawidłowy termin: {error}"))?;

            let target_channel = match channel_id {
                Some(channel_id) => {
                    let channel_id = ChannelId::new(parse_snowflake(channel_id)?);
                    ensure_channel_belongs_to_guild(ctx, channel_id, guild_id).await?;
                    channel_id
                }
                None => execution
                    .channel_id
                    .ok_or_else(|| anyhow!("brakuje kanału docelowego"))?,
            };

            let repeat_every_seconds = repeat
                .as_deref()
                .map(parse_repeat_interval)
                .transpose()
                .map_err(|error| anyhow!("nieprawidłowy interwał powtarzania: {error}"))?;
            let remaining_runs = if repeat_every_seconds.is_some() {
                repeat_count.map(i64::from)
            } else {
                Some(1)
            };

            let new_job = NewScheduledJob {
                guild_id: guild_key.clone(),
                module_id: module_id.to_owned(),
                channel_id: target_channel.get().to_string(),
                creator_user_id: actor_key.clone(),
                content: content.trim().to_owned(),
                mention_creator: *mention_creator,
                run_at,
                repeat_every_seconds,
                remaining_runs,
                max_attempts: 5,
            };

            match state.scheduler.create_job(new_job, *max_jobs).await? {
                CreateJobOutcome::Created(job) => {
                    state
                        .storage
                        .record_audit(
                            Some(&guild_key),
                            Some(&actor_key),
                            module_id,
                            "scheduled_job_created",
                            serde_json::json!({
                                "job_id": job.id,
                                "channel_id": job.channel_id,
                                "run_at": job.run_at,
                                "repeat_every_seconds": job.repeat_every_seconds,
                                "remaining_runs": job.remaining_runs,
                                "mention_creator": job.mention_creator,
                            }),
                        )
                        .await?;

                    Ok(Some(format!(
                        "✅ Utworzono zadanie `{}`.\nKanał: <#{}>\nWykonanie: <t:{}:F> (<t:{}:R>)\nTryb: {}",
                        job.id,
                        job.channel_id,
                        job.run_at.timestamp(),
                        job.run_at.timestamp(),
                        recurrence_description(&job),
                    )))
                }
                CreateJobOutcome::LimitReached { limit } => Ok(Some(format!(
                    "Osiągnięto limit **{limit}** aktywnych zadań dla tego użytkownika."
                ))),
            }
        }
        SchedulerOperation::List { scope, limit } => {
            let include_all = matches!(scope, SchedulerScope::All);
            let jobs = state
                .scheduler
                .list_jobs(&guild_key, &actor_key, include_all, *limit)
                .await?;
            Ok(Some(format_job_list(&jobs, include_all)))
        }
        SchedulerOperation::Cancel { job_id } => {
            mutate_job(
                state,
                module_id,
                &guild_key,
                &actor_key,
                can_manage_guild,
                job_id,
                JobMutation::Cancel,
            )
            .await
        }
        SchedulerOperation::Pause { job_id } => {
            mutate_job(
                state,
                module_id,
                &guild_key,
                &actor_key,
                can_manage_guild,
                job_id,
                JobMutation::Pause,
            )
            .await
        }
        SchedulerOperation::Resume { job_id } => {
            mutate_job(
                state,
                module_id,
                &guild_key,
                &actor_key,
                can_manage_guild,
                job_id,
                JobMutation::Resume,
            )
            .await
        }
    }
}

async fn mutate_job(
    state: &AppState,
    module_id: &str,
    guild_id: &str,
    actor_user_id: &str,
    allow_any: bool,
    job_id: &str,
    mutation: JobMutation,
) -> Result<Option<String>> {
    let outcome = state
        .scheduler
        .mutate_job(
            guild_id,
            job_id,
            actor_user_id,
            allow_any,
            mutation,
        )
        .await?;

    match outcome {
        JobMutationOutcome::Updated(job) => {
            state
                .storage
                .record_audit(
                    Some(guild_id),
                    Some(actor_user_id),
                    module_id,
                    mutation.event_name(),
                    serde_json::json!({
                        "job_id": job.id,
                        "owner_user_id": job.creator_user_id,
                        "status": job.status,
                        "run_at": job.run_at,
                    }),
                )
                .await?;
            Ok(Some(format!(
                "✅ Zadanie `{}` ma teraz status **{}**.",
                job.id,
                status_label(&job.status),
            )))
        }
        JobMutationOutcome::NotFound => Ok(Some(
            "Nie znaleziono zadania o podanym identyfikatorze na tym serwerze.".to_owned(),
        )),
        JobMutationOutcome::Forbidden => Ok(Some(
            "Możesz zarządzać tylko własnymi zadaniami. Moderator z uprawnieniem Zarządzanie serwerem może zarządzać wszystkimi."
                .to_owned(),
        )),
        JobMutationOutcome::InvalidState { current_status } => Ok(Some(format!(
            "Ta zmiana nie jest dozwolona dla zadania w stanie **{}**.",
            status_label(&current_status),
        ))),
    }
}

async fn ensure_channel_belongs_to_guild(
    ctx: &Context,
    channel_id: ChannelId,
    expected_guild_id: GuildId,
) -> Result<()> {
    let channel = channel_id
        .to_channel(ctx)
        .await
        .with_context(|| format!("nie można odczytać kanału {channel_id}"))?;
    match channel {
        Channel::Guild(channel) if channel.guild_id == expected_guild_id => Ok(()),
        Channel::Guild(_) => Err(anyhow!(
            "kanał docelowy musi należeć do serwera, na którym wywołano komendę"
        )),
        _ => Err(anyhow!("kanał prywatny nie może być celem harmonogramu")),
    }
}

fn format_job_list(jobs: &[ScheduledJob], include_owner: bool) -> String {
    if jobs.is_empty() {
        return "Brak aktywnych, wstrzymanych lub wykonywanych zadań.".to_owned();
    }

    let mut lines = Vec::with_capacity(jobs.len() + 1);
    lines.push(format!("🗓️ **Zaplanowane zadania ({})**", jobs.len()));
    for job in jobs {
        let mut preview = job.content.replace(['\n', '\r'], " ");
        if preview.chars().count() > 56 {
            preview = preview.chars().take(53).collect::<String>() + "...";
        }
        let owner = if include_owner {
            format!(" · <@{}>", job.creator_user_id)
        } else {
            String::new()
        };
        lines.push(format!(
            "• `{}` · **{}** · <t:{}:R> · <#{}>{owner}\n  `{}` · {}",
            job.short_id(),
            status_label(&job.status),
            job.run_at.timestamp(),
            job.channel_id,
            job.id,
            preview,
        ));
    }
    lines.join("\n")
}

fn recurrence_description(job: &ScheduledJob) -> String {
    let Some(seconds) = job.repeat_every_seconds else {
        return "jednorazowo".to_owned();
    };
    match job.remaining_runs {
        Some(runs) => format!("co {seconds} s, łącznie {runs} wykonań"),
        None => format!("co {seconds} s, bez limitu wykonań"),
    }
}

fn status_label(status: &str) -> &str {
    match status {
        "active" => "aktywne",
        "paused" => "wstrzymane",
        "processing" => "wykonywane",
        "completed" => "ukończone",
        "cancelled" => "anulowane",
        "failed" => "nieudane",
        _ => status,
    }
}

fn has_manage_guild(permissions: Permissions) -> bool {
    permissions.contains(Permissions::ADMINISTRATOR)
        || permissions.contains(Permissions::MANAGE_GUILD)
}

fn parse_snowflake(value: &str) -> Result<u64> {
    let value = value
        .parse::<u64>()
        .with_context(|| format!("{value:?} nie jest identyfikatorem Discorda"))?;
    if value == 0 {
        return Err(anyhow!("identyfikator Discorda nie może być zerem"));
    }
    Ok(value)
}

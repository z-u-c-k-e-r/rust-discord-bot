use anyhow::{Result, anyhow};
use serenity::{
    all::{ChannelId, GuildId, Permissions, UserId},
    builder::{CreateAllowedMentions, CreateMessage},
    client::Context,
};

use crate::{
    AppState,
    lua::{ProgressionMetric as LuaProgressionMetric, ProgressionOperation},
    storage::{
        CoinTransferOutcome, DailyClaimOutcome, ProgressMetric, ReputationGrantOutcome,
        total_xp_for_level,
    },
};

#[derive(Clone, Copy)]
pub(super) struct ProgressionExecutionContext {
    guild_id: Option<GuildId>,
    channel_id: Option<ChannelId>,
    actor_id: Option<UserId>,
    actor_permissions: Permissions,
    command_context: bool,
}

impl ProgressionExecutionContext {
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
    execution: ProgressionExecutionContext,
    operation: &ProgressionOperation,
) -> Result<Option<String>> {
    let ProgressionExecutionContext {
        guild_id,
        channel_id,
        actor_id,
        actor_permissions,
        command_context,
    } = execution;
    if module_id != "progression" {
        return Err(anyhow!(
            "operacje progression są zarezerwowane dla zaufanego modułu progression"
        ));
    }
    if operation.requires_manage_guild()
        && (!command_context || !has_manage_guild(actor_permissions))
    {
        return Err(anyhow!(
            "korekta progression wymaga komendy użytkownika z uprawnieniem Zarządzanie serwerem"
        ));
    }

    let guild_id = guild_id.ok_or_else(|| anyhow!("ta operacja wymaga serwera"))?;
    let actor_id = actor_id.ok_or_else(|| anyhow!("ta operacja wymaga użytkownika"))?;
    let guild_key = guild_id.get().to_string();
    let actor_key = actor_id.get().to_string();

    match operation {
        ProgressionOperation::AwardMessageXp {
            amount,
            cooldown_seconds,
            announce_level_up,
            level_up_message,
        } => {
            let award = state
                .storage
                .award_message_xp(
                    &guild_key,
                    &actor_key,
                    i64::from(*amount),
                    *cooldown_seconds,
                )
                .await?;

            if award.leveled_up() {
                state
                    .storage
                    .record_audit(
                        Some(&guild_key),
                        Some(&actor_key),
                        module_id,
                        "member_level_up",
                        serde_json::json!({
                            "user_id": actor_key,
                            "previous_level": award.previous_level,
                            "level": award.profile.level,
                            "xp": award.profile.xp,
                        }),
                    )
                    .await?;

                if *announce_level_up && let Some(channel_id) = channel_id {
                    let template = level_up_message.as_deref().unwrap_or(
                        "🎉 {user} awansuje na poziom **{level}**! Łączne XP: **{xp}**.",
                    );
                    let content = template
                        .replace("{user}", &format!("<@{}>", actor_id.get()))
                        .replace("{level}", &award.profile.level.to_string())
                        .replace("{xp}", &award.profile.xp.to_string());
                    channel_id
                        .send_message(
                            &ctx.http,
                            CreateMessage::new()
                                .content(content)
                                .allowed_mentions(CreateAllowedMentions::new()),
                        )
                        .await?;
                }
            }

            Ok(None)
        }
        ProgressionOperation::Profile { user_id } => {
            let target = user_id.as_deref().unwrap_or(&actor_key);
            let profile = state
                .storage
                .get_member_progress(&guild_key, target)
                .await?;
            let level_floor = total_xp_for_level(profile.level);
            let next_level = total_xp_for_level(profile.level.saturating_add(1));
            let current_progress = profile.xp.saturating_sub(level_floor);
            let required_progress = next_level.saturating_sub(level_floor).max(1);

            Ok(Some(format!(
                "👤 **Profil <@{target}>**\nPoziom: **{}**\nXP: **{}** / **{}** do kolejnego poziomu\nMonety: **{}**\nReputacja: **{}**\nWiadomości: **{}**\nSeria daily: **{}**",
                profile.level,
                current_progress,
                required_progress,
                profile.coins,
                profile.reputation,
                profile.messages,
                profile.daily_streak,
            )))
        }
        ProgressionOperation::Balance { user_id } => {
            let target = user_id.as_deref().unwrap_or(&actor_key);
            let profile = state
                .storage
                .get_member_progress(&guild_key, target)
                .await?;
            Ok(Some(format!(
                "🪙 **Saldo <@{target}>:** {} monet\nSeria daily: **{}**",
                profile.coins, profile.daily_streak,
            )))
        }
        ProgressionOperation::Leaderboard { metric, limit } => {
            let metric = storage_metric(*metric);
            let profiles = state
                .storage
                .progression_leaderboard(&guild_key, metric, *limit)
                .await?;

            if profiles.is_empty() {
                return Ok(Some("Ranking jest jeszcze pusty.".to_owned()));
            }

            let mut lines = Vec::with_capacity(profiles.len() + 1);
            lines.push(format!("🏆 **Ranking według {}**", metric.label()));
            for (index, profile) in profiles.iter().enumerate() {
                let position = match index {
                    0 => "🥇".to_owned(),
                    1 => "🥈".to_owned(),
                    2 => "🥉".to_owned(),
                    _ => format!("`{}.`", index + 1),
                };
                lines.push(format!(
                    "{position} <@{}> — **{}** {}",
                    profile.user_id,
                    metric.value(profile),
                    metric.label(),
                ));
            }
            Ok(Some(lines.join("\n")))
        }
        ProgressionOperation::Daily {
            base_reward,
            streak_bonus,
            max_streak_bonus,
        } => {
            let outcome = state
                .storage
                .claim_daily(
                    &guild_key,
                    &actor_key,
                    i64::from(*base_reward),
                    i64::from(*streak_bonus),
                    i32::from(*max_streak_bonus),
                )
                .await?;

            match outcome {
                DailyClaimOutcome::Claimed {
                    profile,
                    reward,
                    next_claim_at,
                } => {
                    state
                        .storage
                        .record_audit(
                            Some(&guild_key),
                            Some(&actor_key),
                            module_id,
                            "daily_claimed",
                            serde_json::json!({
                                "reward": reward,
                                "streak": profile.daily_streak,
                                "balance": profile.coins,
                            }),
                        )
                        .await?;
                    Ok(Some(format!(
                        "🎁 Odbierasz **{reward}** monet.\nSaldo: **{}**\nSeria: **{} dni**\nNastępny daily: <t:{}:R>",
                        profile.coins,
                        profile.daily_streak,
                        next_claim_at.timestamp(),
                    )))
                }
                DailyClaimOutcome::Cooldown {
                    profile,
                    next_claim_at,
                } => Ok(Some(format!(
                    "Daily jest jeszcze niedostępny. Następna nagroda: <t:{}:R>.\nAktualne saldo: **{}** monet.",
                    next_claim_at.timestamp(),
                    profile.coins,
                ))),
            }
        }
        ProgressionOperation::Transfer { user_id, amount } => {
            let amount = i64::try_from(*amount).map_err(|_| anyhow!("kwota jest zbyt duża"))?;
            match state
                .storage
                .transfer_coins(&guild_key, &actor_key, user_id, amount)
                .await?
            {
                CoinTransferOutcome::Completed(transfer) => {
                    state
                        .storage
                        .record_audit(
                            Some(&guild_key),
                            Some(&actor_key),
                            module_id,
                            "coins_transferred",
                            serde_json::json!({
                                "recipient_user_id": user_id,
                                "amount": transfer.amount,
                                "sender_balance": transfer.sender.coins,
                                "recipient_balance": transfer.recipient.coins,
                            }),
                        )
                        .await?;
                    Ok(Some(format!(
                        "✅ Przekazano **{}** monet użytkownikowi <@{}>. Twoje saldo: **{}**.",
                        transfer.amount, user_id, transfer.sender.coins,
                    )))
                }
                CoinTransferOutcome::InsufficientFunds { balance } => Ok(Some(format!(
                    "Masz za mało monet. Aktualne saldo: **{balance}**."
                ))),
                CoinTransferOutcome::SameAccount => {
                    Ok(Some("Nie możesz wysłać monet samemu sobie.".to_owned()))
                }
            }
        }
        ProgressionOperation::GiveReputation {
            user_id,
            amount,
            cooldown_seconds,
        } => {
            match state
                .storage
                .give_reputation(
                    &guild_key,
                    &actor_key,
                    user_id,
                    i64::from(*amount),
                    *cooldown_seconds,
                )
                .await?
            {
                ReputationGrantOutcome::Granted {
                    profile,
                    amount,
                    next_available_at,
                } => {
                    state
                        .storage
                        .record_audit(
                            Some(&guild_key),
                            Some(&actor_key),
                            module_id,
                            "reputation_given",
                            serde_json::json!({
                                "target_user_id": user_id,
                                "amount": amount,
                                "target_reputation": profile.reputation,
                            }),
                        )
                        .await?;
                    Ok(Some(format!(
                        "💚 Dodano **{amount}** reputacji użytkownikowi <@{user_id}>. Łączna reputacja: **{}**.\nKolejna możliwość: <t:{}:R>.",
                        profile.reputation,
                        next_available_at.timestamp(),
                    )))
                }
                ReputationGrantOutcome::Cooldown { next_available_at } => Ok(Some(format!(
                    "Reputację możesz przyznać ponownie <t:{}:R>.",
                    next_available_at.timestamp(),
                ))),
                ReputationGrantOutcome::SameAccount => Ok(Some(
                    "Nie możesz przyznać reputacji samemu sobie.".to_owned(),
                )),
            }
        }
        ProgressionOperation::Adjust {
            user_id,
            xp_delta,
            coins_delta,
            reputation_delta,
            reason,
        } => {
            let profile = state
                .storage
                .adjust_member_progress(
                    &guild_key,
                    user_id,
                    *xp_delta,
                    *coins_delta,
                    *reputation_delta,
                )
                .await?;
            state
                .storage
                .record_audit(
                    Some(&guild_key),
                    Some(&actor_key),
                    module_id,
                    "progress_adjusted",
                    serde_json::json!({
                        "target_user_id": user_id,
                        "xp_delta": xp_delta,
                        "coins_delta": coins_delta,
                        "reputation_delta": reputation_delta,
                        "reason": reason,
                        "result": {
                            "xp": profile.xp,
                            "level": profile.level,
                            "coins": profile.coins,
                            "reputation": profile.reputation,
                        }
                    }),
                )
                .await?;

            Ok(Some(format!(
                "✅ Zaktualizowano <@{user_id}>: poziom **{}**, XP **{}**, monety **{}**, reputacja **{}**.",
                profile.level, profile.xp, profile.coins, profile.reputation,
            )))
        }
    }
}

const fn storage_metric(metric: LuaProgressionMetric) -> ProgressMetric {
    match metric {
        LuaProgressionMetric::Xp => ProgressMetric::Xp,
        LuaProgressionMetric::Coins => ProgressMetric::Coins,
        LuaProgressionMetric::Reputation => ProgressMetric::Reputation,
        LuaProgressionMetric::Messages => ProgressMetric::Messages,
    }
}

fn has_manage_guild(permissions: Permissions) -> bool {
    permissions.contains(Permissions::ADMINISTRATOR)
        || permissions.contains(Permissions::MANAGE_GUILD)
}

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};

use super::{
    CoinTransfer, CoinTransferOutcome, DailyClaimOutcome, GuildModuleSettings, MemberProgress,
    ProgressMetric, ReputationGrantOutcome, XpAward, level_for_xp,
};

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn get_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> anyhow::Result<Option<GuildModuleSettings>> {
        let result = sqlx::query_as::<_, GuildModuleSettings>(
            r#"
            SELECT guild_id, module_id, enabled, config, updated_at
            FROM guild_module_settings
            WHERE guild_id = $1 AND module_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn set_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
        enabled: bool,
        config: Value,
    ) -> anyhow::Result<GuildModuleSettings> {
        let result = sqlx::query_as::<_, GuildModuleSettings>(
            r#"
            INSERT INTO guild_module_settings (guild_id, module_id, enabled, config)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (guild_id, module_id)
            DO UPDATE SET
                enabled = EXCLUDED.enabled,
                config = EXCLUDED.config,
                updated_at = NOW()
            RETURNING guild_id, module_id, enabled, config, updated_at
            "#,
        )
        .bind(guild_id)
        .bind(module_id)
        .bind(enabled)
        .bind(config)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_events (guild_id, actor_id, module_id, event, data)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(guild_id)
        .bind(actor_id)
        .bind(module_id)
        .bind(event)
        .bind(data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_member_progress(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> anyhow::Result<MemberProgress> {
        ensure_profile_on_pool(&self.pool, guild_id, user_id).await?;
        Ok(fetch_profile_on_pool(&self.pool, guild_id, user_id).await?)
    }

    pub async fn award_message_xp(
        &self,
        guild_id: &str,
        user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> anyhow::Result<XpAward> {
        let mut transaction = self.pool.begin().await?;
        ensure_profile(&mut transaction, guild_id, user_id).await?;
        let mut profile = lock_profile(&mut transaction, guild_id, user_id).await?;
        let now = Utc::now();
        let previous_level = profile.level;
        profile.messages = profile.messages.saturating_add(1);

        let eligible = profile.last_xp_at.as_ref().is_none_or(|last_award| {
            now.signed_duration_since(*last_award) >= Duration::seconds(i64::from(cooldown_seconds))
        });
        let awarded = if eligible { amount.max(0) } else { 0 };
        if awarded > 0 {
            profile.xp = profile.xp.saturating_add(awarded);
            profile.level = level_for_xp(profile.xp);
            profile.last_xp_at = Some(now);
        }
        profile.updated_at = now;
        write_profile(&mut transaction, &profile).await?;
        transaction.commit().await?;

        Ok(XpAward {
            profile,
            awarded,
            previous_level,
        })
    }

    pub async fn claim_daily(
        &self,
        guild_id: &str,
        user_id: &str,
        base_reward: i64,
        streak_bonus: i64,
        max_streak_bonus: i32,
    ) -> anyhow::Result<DailyClaimOutcome> {
        let mut transaction = self.pool.begin().await?;
        ensure_profile(&mut transaction, guild_id, user_id).await?;
        let mut profile = lock_profile(&mut transaction, guild_id, user_id).await?;
        let now = Utc::now();

        if let Some(last_claim) = profile.last_daily_at {
            let next_claim_at = last_claim + Duration::hours(24);
            if now < next_claim_at {
                transaction.commit().await?;
                return Ok(DailyClaimOutcome::Cooldown {
                    profile,
                    next_claim_at,
                });
            }

            profile.daily_streak = if now <= last_claim + Duration::hours(48) {
                profile.daily_streak.saturating_add(1).max(1)
            } else {
                1
            };
        } else {
            profile.daily_streak = 1;
        }

        let bonus_steps = profile
            .daily_streak
            .saturating_sub(1)
            .min(max_streak_bonus.max(0));
        let reward = base_reward
            .max(0)
            .saturating_add(streak_bonus.max(0).saturating_mul(i64::from(bonus_steps)));
        profile.coins = profile.coins.saturating_add(reward);
        profile.last_daily_at = Some(now);
        profile.updated_at = now;
        write_profile(&mut transaction, &profile).await?;
        transaction.commit().await?;

        Ok(DailyClaimOutcome::Claimed {
            profile,
            reward,
            next_claim_at: now + Duration::hours(24),
        })
    }

    pub async fn transfer_coins(
        &self,
        guild_id: &str,
        sender_user_id: &str,
        recipient_user_id: &str,
        amount: i64,
    ) -> anyhow::Result<CoinTransferOutcome> {
        if sender_user_id == recipient_user_id {
            return Ok(CoinTransferOutcome::SameAccount);
        }

        let mut transaction = self.pool.begin().await?;
        ensure_profile(&mut transaction, guild_id, sender_user_id).await?;
        ensure_profile(&mut transaction, guild_id, recipient_user_id).await?;

        let (first_user_id, second_user_id) = if sender_user_id < recipient_user_id {
            (sender_user_id, recipient_user_id)
        } else {
            (recipient_user_id, sender_user_id)
        };
        let first = lock_profile(&mut transaction, guild_id, first_user_id).await?;
        let second = lock_profile(&mut transaction, guild_id, second_user_id).await?;
        let (mut sender, mut recipient) = if first.user_id == sender_user_id {
            (first, second)
        } else {
            (second, first)
        };

        if amount <= 0 || sender.coins < amount {
            let balance = sender.coins;
            transaction.commit().await?;
            return Ok(CoinTransferOutcome::InsufficientFunds { balance });
        }

        let now = Utc::now();
        sender.coins -= amount;
        recipient.coins = recipient.coins.saturating_add(amount);
        sender.updated_at = now;
        recipient.updated_at = now;
        write_profile(&mut transaction, &sender).await?;
        write_profile(&mut transaction, &recipient).await?;
        transaction.commit().await?;

        Ok(CoinTransferOutcome::Completed(CoinTransfer {
            sender,
            recipient,
            amount,
        }))
    }

    pub async fn give_reputation(
        &self,
        guild_id: &str,
        giver_user_id: &str,
        target_user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> anyhow::Result<ReputationGrantOutcome> {
        if giver_user_id == target_user_id {
            return Ok(ReputationGrantOutcome::SameAccount);
        }

        let mut transaction = self.pool.begin().await?;
        ensure_profile(&mut transaction, guild_id, target_user_id).await?;
        sqlx::query(
            r#"
            INSERT INTO reputation_cooldowns (guild_id, giver_user_id)
            VALUES ($1, $2)
            ON CONFLICT (guild_id, giver_user_id) DO NOTHING
            "#,
        )
        .bind(guild_id)
        .bind(giver_user_id)
        .execute(&mut *transaction)
        .await?;

        let last_given_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            r#"
            SELECT last_given_at
            FROM reputation_cooldowns
            WHERE guild_id = $1 AND giver_user_id = $2
            FOR UPDATE
            "#,
        )
        .bind(guild_id)
        .bind(giver_user_id)
        .fetch_one(&mut *transaction)
        .await?;
        let now = Utc::now();

        if let Some(last_given_at) = last_given_at {
            let next_available_at = last_given_at + Duration::seconds(i64::from(cooldown_seconds));
            if now < next_available_at {
                transaction.commit().await?;
                return Ok(ReputationGrantOutcome::Cooldown { next_available_at });
            }
        }

        let mut profile = lock_profile(&mut transaction, guild_id, target_user_id).await?;
        let amount = amount.max(0);
        profile.reputation = profile.reputation.saturating_add(amount);
        profile.updated_at = now;
        write_profile(&mut transaction, &profile).await?;
        sqlx::query(
            r#"
            UPDATE reputation_cooldowns
            SET target_user_id = $3, last_given_at = $4
            WHERE guild_id = $1 AND giver_user_id = $2
            "#,
        )
        .bind(guild_id)
        .bind(giver_user_id)
        .bind(target_user_id)
        .bind(now)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(ReputationGrantOutcome::Granted {
            profile,
            amount,
            next_available_at: now + Duration::seconds(i64::from(cooldown_seconds)),
        })
    }

    pub async fn adjust_member_progress(
        &self,
        guild_id: &str,
        user_id: &str,
        xp_delta: i64,
        coins_delta: i64,
        reputation_delta: i64,
    ) -> anyhow::Result<MemberProgress> {
        let mut transaction = self.pool.begin().await?;
        ensure_profile(&mut transaction, guild_id, user_id).await?;
        let mut profile = lock_profile(&mut transaction, guild_id, user_id).await?;
        profile.xp = profile.xp.saturating_add(xp_delta).max(0);
        profile.coins = profile.coins.saturating_add(coins_delta).max(0);
        profile.reputation = profile.reputation.saturating_add(reputation_delta).max(0);
        profile.level = level_for_xp(profile.xp);
        profile.updated_at = Utc::now();
        write_profile(&mut transaction, &profile).await?;
        transaction.commit().await?;
        Ok(profile)
    }

    pub async fn progression_leaderboard(
        &self,
        guild_id: &str,
        metric: ProgressMetric,
        limit: u8,
    ) -> anyhow::Result<Vec<MemberProgress>> {
        let query = match metric {
            ProgressMetric::Xp => {
                "SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak, last_xp_at, last_daily_at, created_at, updated_at FROM member_progress WHERE guild_id = $1 AND xp > 0 ORDER BY xp DESC, user_id ASC LIMIT $2"
            }
            ProgressMetric::Coins => {
                "SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak, last_xp_at, last_daily_at, created_at, updated_at FROM member_progress WHERE guild_id = $1 AND coins > 0 ORDER BY coins DESC, user_id ASC LIMIT $2"
            }
            ProgressMetric::Reputation => {
                "SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak, last_xp_at, last_daily_at, created_at, updated_at FROM member_progress WHERE guild_id = $1 AND reputation > 0 ORDER BY reputation DESC, user_id ASC LIMIT $2"
            }
            ProgressMetric::Messages => {
                "SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak, last_xp_at, last_daily_at, created_at, updated_at FROM member_progress WHERE guild_id = $1 AND messages > 0 ORDER BY messages DESC, user_id ASC LIMIT $2"
            }
        };

        Ok(sqlx::query_as::<_, MemberProgress>(query)
            .bind(guild_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?)
    }
}

async fn ensure_profile_on_pool(
    pool: &PgPool,
    guild_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO member_progress (guild_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (guild_id, user_id) DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_profile_on_pool(
    pool: &PgPool,
    guild_id: &str,
    user_id: &str,
) -> Result<MemberProgress, sqlx::Error> {
    sqlx::query_as::<_, MemberProgress>(
        r#"
        SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak,
               last_xp_at, last_daily_at, created_at, updated_at
        FROM member_progress
        WHERE guild_id = $1 AND user_id = $2
        "#,
    )
    .bind(guild_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

async fn ensure_profile(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO member_progress (guild_id, user_id)
        VALUES ($1, $2)
        ON CONFLICT (guild_id, user_id) DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn lock_profile(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    user_id: &str,
) -> Result<MemberProgress, sqlx::Error> {
    sqlx::query_as::<_, MemberProgress>(
        r#"
        SELECT guild_id, user_id, xp, level, coins, reputation, messages, daily_streak,
               last_xp_at, last_daily_at, created_at, updated_at
        FROM member_progress
        WHERE guild_id = $1 AND user_id = $2
        FOR UPDATE
        "#,
    )
    .bind(guild_id)
    .bind(user_id)
    .fetch_one(&mut **transaction)
    .await
}

async fn write_profile(
    transaction: &mut Transaction<'_, Postgres>,
    profile: &MemberProgress,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE member_progress
        SET xp = $3,
            level = $4,
            coins = $5,
            reputation = $6,
            messages = $7,
            daily_streak = $8,
            last_xp_at = $9,
            last_daily_at = $10,
            updated_at = $11
        WHERE guild_id = $1 AND user_id = $2
        "#,
    )
    .bind(&profile.guild_id)
    .bind(&profile.user_id)
    .bind(profile.xp)
    .bind(profile.level)
    .bind(profile.coins)
    .bind(profile.reputation)
    .bind(profile.messages)
    .bind(profile.daily_streak)
    .bind(profile.last_xp_at)
    .bind(profile.last_daily_at)
    .bind(profile.updated_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

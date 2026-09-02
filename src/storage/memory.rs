use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{Duration, Utc};
use dashmap::DashMap;
use serde_json::Value;

use super::{
    CoinTransfer, CoinTransferOutcome, DailyClaimOutcome, GuildModuleSettings, MemberProgress,
    ProgressMetric, ReputationGrantOutcome, XpAward, level_for_xp,
};

type ModuleKey = (String, String);
type MemberKey = (String, String);
type ReputationKey = (String, String);

#[derive(Clone, Default)]
pub struct MemoryStore {
    settings: Arc<DashMap<ModuleKey, GuildModuleSettings>>,
    audit_log: Arc<DashMap<u64, Value>>,
    next_audit_id: Arc<std::sync::atomic::AtomicU64>,
    progression: Arc<DashMap<MemberKey, MemberProgress>>,
    reputation_cooldowns: Arc<DashMap<ReputationKey, chrono::DateTime<Utc>>>,
    progression_lock: Arc<Mutex<()>>,
}

impl MemoryStore {
    pub fn get_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> Option<GuildModuleSettings> {
        self.settings
            .get(&(guild_id.to_owned(), module_id.to_owned()))
            .map(|entry| entry.value().clone())
    }

    pub fn set_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
        enabled: bool,
        config: Value,
    ) -> GuildModuleSettings {
        let value = GuildModuleSettings {
            guild_id: guild_id.to_owned(),
            module_id: module_id.to_owned(),
            enabled,
            config,
            updated_at: Utc::now(),
        };
        self.settings
            .insert((guild_id.to_owned(), module_id.to_owned()), value.clone());
        value
    }

    pub fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) {
        use std::sync::atomic::Ordering;

        let id = self.next_audit_id.fetch_add(1, Ordering::Relaxed);
        self.audit_log.insert(
            id,
            serde_json::json!({
                "guild_id": guild_id,
                "actor_id": actor_id,
                "module_id": module_id,
                "event": event,
                "data": data,
                "created_at": Utc::now(),
            }),
        );
    }

    pub fn get_member_progress(&self, guild_id: &str, user_id: &str) -> MemberProgress {
        let _guard = self.progression_guard();
        self.profile(guild_id, user_id, Utc::now())
    }

    pub fn award_message_xp(
        &self,
        guild_id: &str,
        user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> XpAward {
        let _guard = self.progression_guard();
        let now = Utc::now();
        let mut profile = self.profile(guild_id, user_id, now);
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
        self.store_profile(profile.clone());

        XpAward {
            profile,
            awarded,
            previous_level,
        }
    }

    pub fn claim_daily(
        &self,
        guild_id: &str,
        user_id: &str,
        base_reward: i64,
        streak_bonus: i64,
        max_streak_bonus: i32,
    ) -> DailyClaimOutcome {
        let _guard = self.progression_guard();
        let now = Utc::now();
        let mut profile = self.profile(guild_id, user_id, now);

        if let Some(last_claim) = profile.last_daily_at {
            let next_claim_at = last_claim + Duration::hours(24);
            if now < next_claim_at {
                return DailyClaimOutcome::Cooldown {
                    profile,
                    next_claim_at,
                };
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
        self.store_profile(profile.clone());

        DailyClaimOutcome::Claimed {
            profile,
            reward,
            next_claim_at: now + Duration::hours(24),
        }
    }

    pub fn transfer_coins(
        &self,
        guild_id: &str,
        sender_user_id: &str,
        recipient_user_id: &str,
        amount: i64,
    ) -> CoinTransferOutcome {
        if sender_user_id == recipient_user_id {
            return CoinTransferOutcome::SameAccount;
        }

        let _guard = self.progression_guard();
        let now = Utc::now();
        let mut sender = self.profile(guild_id, sender_user_id, now);
        if amount <= 0 || sender.coins < amount {
            return CoinTransferOutcome::InsufficientFunds {
                balance: sender.coins,
            };
        }

        let mut recipient = self.profile(guild_id, recipient_user_id, now);
        sender.coins -= amount;
        recipient.coins = recipient.coins.saturating_add(amount);
        sender.updated_at = now;
        recipient.updated_at = now;
        self.store_profile(sender.clone());
        self.store_profile(recipient.clone());

        CoinTransferOutcome::Completed(Box::new(CoinTransfer {
            sender,
            recipient,
            amount,
        }))
    }

    pub fn give_reputation(
        &self,
        guild_id: &str,
        giver_user_id: &str,
        target_user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> ReputationGrantOutcome {
        if giver_user_id == target_user_id {
            return ReputationGrantOutcome::SameAccount;
        }

        let _guard = self.progression_guard();
        let now = Utc::now();
        let cooldown_key = (guild_id.to_owned(), giver_user_id.to_owned());
        if let Some(last_given_at) = self.reputation_cooldowns.get(&cooldown_key) {
            let next_available_at = *last_given_at + Duration::seconds(i64::from(cooldown_seconds));
            if now < next_available_at {
                return ReputationGrantOutcome::Cooldown { next_available_at };
            }
        }

        let mut profile = self.profile(guild_id, target_user_id, now);
        let amount = amount.max(0);
        profile.reputation = profile.reputation.saturating_add(amount);
        profile.updated_at = now;
        self.store_profile(profile.clone());
        self.reputation_cooldowns.insert(cooldown_key, now);

        ReputationGrantOutcome::Granted {
            profile,
            amount,
            next_available_at: now + Duration::seconds(i64::from(cooldown_seconds)),
        }
    }

    pub fn adjust_member_progress(
        &self,
        guild_id: &str,
        user_id: &str,
        xp_delta: i64,
        coins_delta: i64,
        reputation_delta: i64,
    ) -> MemberProgress {
        let _guard = self.progression_guard();
        let now = Utc::now();
        let mut profile = self.profile(guild_id, user_id, now);
        profile.xp = profile.xp.saturating_add(xp_delta).max(0);
        profile.coins = profile.coins.saturating_add(coins_delta).max(0);
        profile.reputation = profile.reputation.saturating_add(reputation_delta).max(0);
        profile.level = level_for_xp(profile.xp);
        profile.updated_at = now;
        self.store_profile(profile.clone());
        profile
    }

    pub fn progression_leaderboard(
        &self,
        guild_id: &str,
        metric: ProgressMetric,
        limit: u8,
    ) -> Vec<MemberProgress> {
        let _guard = self.progression_guard();
        let mut profiles = self
            .progression
            .iter()
            .filter(|entry| entry.value().guild_id == guild_id)
            .map(|entry| entry.value().clone())
            .filter(|profile| metric.value(profile) > 0)
            .collect::<Vec<_>>();

        profiles.sort_by(|left, right| {
            metric
                .value(right)
                .cmp(&metric.value(left))
                .then_with(|| left.user_id.cmp(&right.user_id))
        });
        profiles.truncate(usize::from(limit));
        profiles
    }

    fn progression_guard(&self) -> MutexGuard<'_, ()> {
        self.progression_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn profile(&self, guild_id: &str, user_id: &str, now: chrono::DateTime<Utc>) -> MemberProgress {
        let key = (guild_id.to_owned(), user_id.to_owned());
        self.progression.get(&key).map_or_else(
            || {
                let profile = MemberProgress::empty(guild_id, user_id, now);
                self.progression.insert(key, profile.clone());
                profile
            },
            |entry| entry.value().clone(),
        )
    }

    fn store_profile(&self, profile: MemberProgress) {
        self.progression
            .insert((profile.guild_id.clone(), profile.user_id.clone()), profile);
    }
}

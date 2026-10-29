mod memory;
mod postgres;
mod progression;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use memory::MemoryStore;
pub use postgres::PostgresStore;
pub use progression::{
    CoinTransfer, CoinTransferOutcome, DailyClaimOutcome, MemberProgress, ProgressMetric,
    ReputationGrantOutcome, XpAward, level_for_xp, total_xp_for_level,
};

#[derive(Clone)]
pub enum Storage {
    Memory(MemoryStore),
    Postgres(PostgresStore),
}

#[derive(Clone, Debug)]
pub struct UpdateModuleSettingsParams<'a> {
    pub guild_id: &'a str,
    pub module_id: &'a str,
    pub enabled: bool,
    pub config: Value,
    pub actor_id: Option<&'a str>,
    pub audit_event: &'a str,
    pub audit_data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GuildModuleSettings {
    pub guild_id: String,
    pub module_id: String,
    pub enabled: bool,
    pub config: Value,
    pub updated_at: DateTime<Utc>,
}

impl Storage {
    pub async fn connect(database_url: Option<&str>) -> anyhow::Result<Self> {
        match database_url {
            Some(url) => Ok(Self::Postgres(PostgresStore::connect(url).await?)),
            None => {
                tracing::warn!(
                    "DATABASE_URL is not set; using volatile in-memory configuration storage"
                );
                Ok(Self::Memory(MemoryStore::default()))
            }
        }
    }

    pub async fn get_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
    ) -> anyhow::Result<Option<GuildModuleSettings>> {
        match self {
            Self::Memory(store) => Ok(store.get_module_settings(guild_id, module_id)),
            Self::Postgres(store) => store.get_module_settings(guild_id, module_id).await,
        }
    }

    pub async fn set_module_settings(
        &self,
        guild_id: &str,
        module_id: &str,
        enabled: bool,
        config: Value,
    ) -> anyhow::Result<GuildModuleSettings> {
        match self {
            Self::Memory(store) => {
                Ok(store.set_module_settings(guild_id, module_id, enabled, config))
            }
            Self::Postgres(store) => {
                store
                    .set_module_settings(guild_id, module_id, enabled, config)
                    .await
            }
        }
    }

    pub async fn update_module_settings_with_audit(
        &self,
        params: UpdateModuleSettingsParams<'_>,
    ) -> anyhow::Result<GuildModuleSettings> {
        match self {
            Self::Memory(store) => store.update_module_settings_with_audit(params),
            Self::Postgres(store) => store.update_module_settings_with_audit(params).await,
        }
    }

    pub fn inject_audit_failure(&self, fail: bool) {
        if let Self::Memory(store) = self {
            store.inject_audit_failure(fail);
        }
    }

    pub async fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) -> anyhow::Result<()> {
        match self {
            Self::Memory(store) => store.record_audit(guild_id, actor_id, module_id, event, data),
            Self::Postgres(store) => {
                store
                    .record_audit(guild_id, actor_id, module_id, event, data)
                    .await
            }
        }
    }

    pub async fn get_member_progress(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> anyhow::Result<MemberProgress> {
        match self {
            Self::Memory(store) => Ok(store.get_member_progress(guild_id, user_id)),
            Self::Postgres(store) => store.get_member_progress(guild_id, user_id).await,
        }
    }

    pub async fn award_message_xp(
        &self,
        guild_id: &str,
        user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> anyhow::Result<XpAward> {
        match self {
            Self::Memory(store) => {
                Ok(store.award_message_xp(guild_id, user_id, amount, cooldown_seconds))
            }
            Self::Postgres(store) => {
                store
                    .award_message_xp(guild_id, user_id, amount, cooldown_seconds)
                    .await
            }
        }
    }

    pub async fn claim_daily(
        &self,
        guild_id: &str,
        user_id: &str,
        base_reward: i64,
        streak_bonus: i64,
        max_streak_bonus: i32,
    ) -> anyhow::Result<DailyClaimOutcome> {
        match self {
            Self::Memory(store) => Ok(store.claim_daily(
                guild_id,
                user_id,
                base_reward,
                streak_bonus,
                max_streak_bonus,
            )),
            Self::Postgres(store) => {
                store
                    .claim_daily(
                        guild_id,
                        user_id,
                        base_reward,
                        streak_bonus,
                        max_streak_bonus,
                    )
                    .await
            }
        }
    }

    pub async fn transfer_coins(
        &self,
        guild_id: &str,
        sender_user_id: &str,
        recipient_user_id: &str,
        amount: i64,
    ) -> anyhow::Result<CoinTransferOutcome> {
        match self {
            Self::Memory(store) => {
                Ok(store.transfer_coins(guild_id, sender_user_id, recipient_user_id, amount))
            }
            Self::Postgres(store) => {
                store
                    .transfer_coins(guild_id, sender_user_id, recipient_user_id, amount)
                    .await
            }
        }
    }

    pub async fn give_reputation(
        &self,
        guild_id: &str,
        giver_user_id: &str,
        target_user_id: &str,
        amount: i64,
        cooldown_seconds: u32,
    ) -> anyhow::Result<ReputationGrantOutcome> {
        match self {
            Self::Memory(store) => Ok(store.give_reputation(
                guild_id,
                giver_user_id,
                target_user_id,
                amount,
                cooldown_seconds,
            )),
            Self::Postgres(store) => {
                store
                    .give_reputation(
                        guild_id,
                        giver_user_id,
                        target_user_id,
                        amount,
                        cooldown_seconds,
                    )
                    .await
            }
        }
    }

    pub async fn adjust_member_progress(
        &self,
        guild_id: &str,
        user_id: &str,
        xp_delta: i64,
        coins_delta: i64,
        reputation_delta: i64,
    ) -> anyhow::Result<MemberProgress> {
        match self {
            Self::Memory(store) => Ok(store.adjust_member_progress(
                guild_id,
                user_id,
                xp_delta,
                coins_delta,
                reputation_delta,
            )),
            Self::Postgres(store) => {
                store
                    .adjust_member_progress(
                        guild_id,
                        user_id,
                        xp_delta,
                        coins_delta,
                        reputation_delta,
                    )
                    .await
            }
        }
    }

    pub async fn progression_leaderboard(
        &self,
        guild_id: &str,
        metric: ProgressMetric,
        limit: u8,
    ) -> anyhow::Result<Vec<MemberProgress>> {
        match self {
            Self::Memory(store) => Ok(store.progression_leaderboard(guild_id, metric, limit)),
            Self::Postgres(store) => store.progression_leaderboard(guild_id, metric, limit).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_update_module_settings_with_audit_atomic_success_and_rollback() {
        let store = Storage::Memory(MemoryStore::default());
        let guild_id = "100000000000000001";
        let module_id = "moderation";

        // 1. Successful atomic update
        let initial_config = serde_json::json!({"action": "log"});
        let updated = store
            .update_module_settings_with_audit(UpdateModuleSettingsParams {
                guild_id,
                module_id,
                enabled: true,
                config: initial_config.clone(),
                actor_id: Some("admin_1"),
                audit_event: "module_settings_updated",
                audit_data: serde_json::json!({"module_id": module_id, "enabled": true}),
            })
            .await
            .expect("update should succeed");
        assert!(updated.enabled);
        assert_eq!(updated.config, initial_config);

        let fetched = store
            .get_module_settings(guild_id, module_id)
            .await
            .unwrap()
            .unwrap();
        assert!(fetched.enabled);

        // 2. Inject audit failure
        store.inject_audit_failure(true);
        let new_config = serde_json::json!({"action": "ban"});
        let failed_result = store
            .update_module_settings_with_audit(UpdateModuleSettingsParams {
                guild_id,
                module_id,
                enabled: false,
                config: new_config,
                actor_id: Some("admin_1"),
                audit_event: "module_settings_updated",
                audit_data: serde_json::json!({"module_id": module_id, "enabled": false}),
            })
            .await;

        assert!(failed_result.is_err());

        // 3. Verify rollback: settings must remain exactly as before
        let after_rollback = store
            .get_module_settings(guild_id, module_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            after_rollback.enabled,
            "module must still be enabled due to atomic rollback"
        );
        assert_eq!(
            after_rollback.config, initial_config,
            "config must remain unmutated"
        );

        // 4. Test unconfigured module fresh rollback
        let unconfigured_mod = "automod";
        let failed_fresh = store
            .update_module_settings_with_audit(UpdateModuleSettingsParams {
                guild_id,
                module_id: unconfigured_mod,
                enabled: true,
                config: serde_json::json!({}),
                actor_id: Some("admin_1"),
                audit_event: "module_settings_updated",
                audit_data: serde_json::json!({}),
            })
            .await;
        assert!(failed_fresh.is_err());
        let fetched_fresh = store
            .get_module_settings(guild_id, unconfigured_mod)
            .await
            .unwrap();
        assert!(
            fetched_fresh.is_none(),
            "unconfigured module settings must not exist after rollback"
        );
    }
}

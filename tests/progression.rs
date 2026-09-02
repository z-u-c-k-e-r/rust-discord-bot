use std::path::PathBuf;

use serde_json::json;
use zuckerbot::{
    lua::{
        LuaAction, LuaEngine, LuaEventContext, LuaExecutionContext, LuaLimits, ProgressionOperation,
    },
    storage::{
        CoinTransferOutcome, DailyClaimOutcome, MemoryStore, ProgressMetric, ReputationGrantOutcome,
    },
};

fn engine() -> LuaEngine {
    LuaEngine::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"),
        LuaLimits {
            memory_bytes: 4 * 1024 * 1024,
            instruction_limit: 200_000,
            hook_granularity: 100,
        },
    )
    .expect("bundled Lua modules should load")
}

fn command_context(
    permissions: &str,
    options: serde_json::Value,
    config: serde_json::Value,
) -> LuaExecutionContext {
    LuaExecutionContext {
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: "100000000000000002".to_owned(),
        user_id: "100000000000000003".to_owned(),
        user_name: "Tester".to_owned(),
        member_roles: Vec::new(),
        member_permissions: permissions.to_owned(),
        locale: "pl".to_owned(),
        options,
        config,
    }
}

#[test]
fn memory_store_applies_xp_cooldown_but_counts_messages() {
    let store = MemoryStore::default();
    let first = store.award_message_xp("guild", "member", 100, 3_600);
    let second = store.award_message_xp("guild", "member", 100, 3_600);

    assert_eq!(first.awarded, 100);
    assert_eq!(first.profile.level, 1);
    assert!(first.leveled_up());
    assert_eq!(second.awarded, 0);
    assert_eq!(second.profile.xp, 100);
    assert_eq!(second.profile.messages, 2);
}

#[test]
fn memory_store_transfers_coins_atomically() {
    let store = MemoryStore::default();
    store.adjust_member_progress("guild", "sender", 0, 500, 0);

    let completed = store.transfer_coins("guild", "sender", "recipient", 200);
    match completed {
        CoinTransferOutcome::Completed(transfer) => {
            assert_eq!(transfer.sender.coins, 300);
            assert_eq!(transfer.recipient.coins, 200);
        }
        other => panic!("unexpected transfer result: {other:?}"),
    }

    let rejected = store.transfer_coins("guild", "sender", "recipient", 400);
    assert!(matches!(
        rejected,
        CoinTransferOutcome::InsufficientFunds { balance: 300 }
    ));
    assert_eq!(store.get_member_progress("guild", "sender").coins, 300);
    assert_eq!(store.get_member_progress("guild", "recipient").coins, 200);
}

#[test]
fn memory_store_enforces_daily_and_reputation_cooldowns() {
    let store = MemoryStore::default();

    let daily = store.claim_daily("guild", "member", 100, 10, 30);
    assert!(matches!(
        daily,
        DailyClaimOutcome::Claimed {
            reward: 100,
            profile,
            ..
        } if profile.coins == 100 && profile.daily_streak == 1
    ));
    assert!(matches!(
        store.claim_daily("guild", "member", 100, 10, 30),
        DailyClaimOutcome::Cooldown { .. }
    ));

    assert!(matches!(
        store.give_reputation("guild", "giver", "member", 1, 86_400),
        ReputationGrantOutcome::Granted {
            amount: 1,
            profile,
            ..
        } if profile.reputation == 1
    ));
    assert!(matches!(
        store.give_reputation("guild", "giver", "other", 1, 86_400),
        ReputationGrantOutcome::Cooldown { .. }
    ));
}

#[test]
fn memory_store_builds_metric_specific_leaderboards() {
    let store = MemoryStore::default();
    store.adjust_member_progress("guild", "alpha", 300, 10, 1);
    store.adjust_member_progress("guild", "beta", 100, 500, 2);

    let xp = store.progression_leaderboard("guild", ProgressMetric::Xp, 10);
    let coins = store.progression_leaderboard("guild", ProgressMetric::Coins, 10);
    let reputation = store.progression_leaderboard("guild", ProgressMetric::Reputation, 10);

    assert_eq!(xp[0].user_id, "alpha");
    assert_eq!(coins[0].user_id, "beta");
    assert_eq!(reputation[0].user_id, "beta");
}

#[tokio::test]
async fn lua_progression_module_exposes_commands_and_message_xp() {
    let engine = engine();
    assert_eq!(
        engine.module_for_command("daily").await.as_deref(),
        Some("progression")
    );
    assert!(engine.command_definitions().await.len() >= 50);

    let actions = engine
        .execute_command(
            "progression",
            "daily",
            command_context("0", json!({}), json!({})),
        )
        .await
        .expect("daily should produce a typed progression operation");
    assert!(matches!(
        actions.first(),
        Some(LuaAction::Progression {
            operation: ProgressionOperation::Daily {
                base_reward: 100,
                streak_bonus: 10,
                max_streak_bonus: 30,
            },
        })
    ));

    let event = LuaEventContext {
        name: "message_create".to_owned(),
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: Some("100000000000000002".to_owned()),
        actor_id: Some("100000000000000003".to_owned()),
        data: json!({
            "message_id": "100000000000000004",
            "content": "To jest normalna wiadomość użytkownika."
        }),
        config: json!({
            "xp_min": 15,
            "xp_max": 25,
            "xp_cooldown_seconds": 60
        }),
    };
    let actions = engine
        .execute_event("progression", "message_create", event)
        .await
        .expect("message event should produce XP operation");
    assert!(matches!(
        actions.first(),
        Some(LuaAction::Progression {
            operation: ProgressionOperation::AwardMessageXp {
                amount: 15..=25,
                cooldown_seconds: 60,
                ..
            },
        })
    ));
}

#[tokio::test]
async fn lua_progress_admin_requires_manage_guild_permission() {
    let engine = engine();
    let options = json!({
        "user": "100000000000000004",
        "xp": 100,
        "coins": 0,
        "reputation": 0,
        "reason": "test"
    });

    let denied = engine
        .execute_command(
            "progression",
            "progressadmin",
            command_context("0", options.clone(), json!({})),
        )
        .await
        .expect("Lua permission denial should be a normal response");
    assert!(matches!(
        denied.first(),
        Some(LuaAction::Reply {
            ephemeral: true,
            ..
        })
    ));

    let allowed = engine
        .execute_command(
            "progression",
            "progressadmin",
            command_context("32", options, json!({})),
        )
        .await
        .expect("authorized correction should produce a typed operation");
    assert!(matches!(
        allowed.first(),
        Some(LuaAction::Progression {
            operation: ProgressionOperation::Adjust { xp_delta: 100, .. },
        })
    ));
}

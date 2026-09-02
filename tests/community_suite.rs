use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use zuckerbot::lua::{
    LuaAction, LuaEngine, LuaEventContext, LuaExecutionContext, LuaLimits,
};

fn limits() -> LuaLimits {
    LuaLimits {
        memory_bytes: 4 * 1024 * 1024,
        instruction_limit: 200_000,
        hook_granularity: 100,
    }
}

fn engine() -> LuaEngine {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    LuaEngine::load(scripts, limits()).expect("the complete bundled module suite should load")
}

fn context(options: serde_json::Value, config: serde_json::Value) -> LuaExecutionContext {
    LuaExecutionContext {
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: "100000000000000002".to_owned(),
        user_id: "100000000000000003".to_owned(),
        user_name: "Tester".to_owned(),
        member_roles: Vec::new(),
        member_permissions: "0".to_owned(),
        locale: "pl".to_owned(),
        options,
        config,
    }
}

#[tokio::test]
async fn bundled_suite_registers_expected_modules_and_commands() {
    let engine = engine();
    let manifests = engine.manifests().await;
    let commands = engine.command_definitions().await;

    assert!(manifests.len() >= 13, "expected the expanded module suite");
    assert!(commands.len() >= 43, "expected the expanded command catalog");

    for (command, module_id) in [
        ("calculate", "utility"),
        ("coinflip", "games"),
        ("suggest", "community"),
        ("announce", "staff_tools"),
        ("rules", "server_info"),
    ] {
        assert_eq!(
            engine.module_for_command(command).await.as_deref(),
            Some(module_id),
            "command /{command} should be owned by {module_id}"
        );
    }
}

#[tokio::test]
async fn calculator_executes_inside_the_lua_sandbox() {
    let actions = engine()
        .execute_command(
            "utility",
            "calculate",
            context(
                json!({ "a": 6, "operation": "multiply", "b": 7 }),
                json!({}),
            ),
        )
        .await
        .expect("calculator command should execute");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::Reply {
            content,
            ephemeral: false,
        }) if content.contains("42")
    ));
}

#[tokio::test]
async fn report_routes_only_to_the_configured_staff_channel() {
    let actions = engine()
        .execute_command(
            "community",
            "report",
            context(
                json!({
                    "user": "100000000000000004",
                    "reason": "Testowe zgłoszenie naruszenia regulaminu",
                    "evidence": "100000000000000005"
                }),
                json!({ "report_channel_id": "100000000000000006" }),
            ),
        )
        .await
        .expect("report command should execute");

    assert_eq!(actions.len(), 3);
    assert!(matches!(
        &actions[1],
        LuaAction::SendMessage {
            channel_id: Some(channel_id),
            content: _,
        } if channel_id == "100000000000000006"
    ));
    assert!(matches!(actions[2], LuaAction::Audit { .. }));
}

#[tokio::test]
async fn staff_tools_reject_users_without_permissions() {
    let actions = engine()
        .execute_command(
            "staff_tools",
            "announce",
            context(
                json!({
                    "channel": "100000000000000006",
                    "title": "Test",
                    "message": "Treść"
                }),
                json!({}),
            ),
        )
        .await
        .expect("permission denial should be a normal Lua response");

    assert_eq!(actions.len(), 1);
    assert!(matches!(
        actions.first(),
        Some(LuaAction::Reply {
            content,
            ephemeral: true,
        }) if content.contains("uprawnień")
    ));
}

#[tokio::test]
async fn automod_detects_case_insensitive_blocked_phrases() {
    let event = LuaEventContext {
        name: "message_create".to_owned(),
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: Some("100000000000000002".to_owned()),
        actor_id: Some("100000000000000003".to_owned()),
        data: json!({
            "message_id": "100000000000000004",
            "content": "To zawiera ZABLOKOWANA FRAZA w środku",
            "mentions": [],
            "attachments": []
        }),
        config: json!({
            "blocked_words": ["zablokowana fraza"],
            "delete_messages": true,
            "send_notice": false
        }),
    };

    let actions = engine()
        .execute_event("automod", "message_create", event)
        .await
        .expect("automod event should execute");

    assert!(matches!(actions.first(), Some(LuaAction::DeleteMessage { .. })));
    assert!(matches!(actions.last(), Some(LuaAction::Audit { .. })));
}

#[tokio::test]
async fn join_guard_flags_a_fresh_discord_account() {
    const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_millis() as u64;
    let user_id = ((now_ms - DISCORD_EPOCH_MS) << 22).to_string();

    let event = LuaEventContext {
        name: "guild_member_add".to_owned(),
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: None,
        actor_id: Some(user_id.clone()),
        data: json!({
            "user": {
                "id": user_id,
                "name": "FreshAccount",
                "global_name": "Fresh Account",
                "bot": false
            },
            "roles": [],
            "joined_at": null
        }),
        config: json!({
            "alert_channel_id": "100000000000000006",
            "minimum_account_age_days": 7
        }),
    };

    let actions = engine()
        .execute_event("join_guard", "guild_member_add", event)
        .await
        .expect("join guard event should execute");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::SendMessage {
            channel_id: Some(channel_id),
            content: _,
        }) if channel_id == "100000000000000006"
    ));
    assert!(matches!(actions.last(), Some(LuaAction::Audit { .. })));
}

use std::path::PathBuf;

use serde_json::json;
use tempfile::TempDir;
use zuckerbot::lua::{LuaAction, LuaEngine, LuaExecutionContext, LuaLimits};

fn limits() -> LuaLimits {
    LuaLimits {
        memory_bytes: 2 * 1024 * 1024,
        instruction_limit: 100_000,
        hook_granularity: 100,
    }
}

fn context() -> LuaExecutionContext {
    LuaExecutionContext {
        guild_id: Some("100000000000000001".to_owned()),
        channel_id: "100000000000000002".to_owned(),
        user_id: "100000000000000003".to_owned(),
        user_name: "Tester".to_owned(),
        member_roles: Vec::new(),
        member_permissions: "0".to_owned(),
        locale: "pl".to_owned(),
        options: json!({}),
        config: json!({}),
    }
}

#[tokio::test]
async fn bundled_modules_load_and_ping_executes() {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    let engine = LuaEngine::load(scripts, limits()).expect("bundled scripts should load");

    let manifests = engine.manifests().await;
    assert!(manifests.len() >= 7);
    assert_eq!(
        engine.module_for_command("ping").await.as_deref(),
        Some("core")
    );

    let actions = engine
        .execute_command("core", "ping", context())
        .await
        .expect("ping should execute");

    assert!(matches!(
        actions.first(),
        Some(LuaAction::Reply {
            content: _,
            ephemeral: true
        })
    ));
}

#[tokio::test]
async fn moderation_warning_returns_a_persistent_case_with_escalation_rules() {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    let engine = LuaEngine::load(scripts, limits()).expect("bundled scripts should load");
    let mut command_context = context();
    command_context.options = json!({
        "user": "100000000000000010",
        "reason": "Repeated spam",
        "points": 2,
        "expires_days": 30
    });
    command_context.config = json!({
        "timeout_at_points": 3,
        "escalation_timeout_seconds": 3600,
        "kick_at_points": 7,
        "ban_at_points": 10,
        "ban_delete_message_days": 1
    });

    let actions = engine
        .execute_command("moderation", "warn", command_context)
        .await
        .expect("warning should execute");

    assert!(actions.iter().any(|action| {
        matches!(
            action,
            LuaAction::CreateModerationCase {
                target_user_id,
                case_type,
                points,
                expires_in_seconds: Some(2_592_000),
                escalation_rules,
                ..
            } if target_user_id == "100000000000000010"
                && case_type == "warning"
                && *points == 2
                && escalation_rules.len() == 3
        )
    }));
}

#[tokio::test]
async fn infinite_loop_hits_instruction_limit() {
    let directory = TempDir::new().expect("temporary directory");
    std::fs::write(
        directory.path().join("loop.lua"),
        r#"
        return {
            manifest = {
                id = "loop",
                name = "Loop",
                version = "1.0.0",
                description = "Instruction limit test module.",
                category = "test",
                commands = {
                    { name = "loop", description = "Runs an endless loop." }
                }
            },
            on_command = function()
                while true do end
            end
        }
        "#,
    )
    .expect("write test module");

    let engine = LuaEngine::load(directory.path(), limits()).expect("module should load");
    let result = engine.execute_command("loop", "loop", context()).await;

    assert!(result.is_err(), "endless scripts must be interrupted");
    assert!(
        result
            .expect_err("expected instruction-limit failure")
            .to_string()
            .contains("instruction limit")
    );
}

#[tokio::test]
async fn dangerous_lua_globals_are_unavailable() {
    let directory = TempDir::new().expect("temporary directory");
    std::fs::write(
        directory.path().join("sandbox.lua"),
        r#"
        return {
            manifest = {
                id = "sandbox",
                name = "Sandbox",
                version = "1.0.0",
                description = "Sandbox global-denylist test module.",
                category = "test",
                commands = {
                    { name = "sandbox", description = "Checks denied Lua globals." }
                }
            },
            on_command = function()
                assert(collectgarbage == nil)
                assert(coroutine == nil)
                assert(debug == nil)
                assert(dofile == nil)
                assert(io == nil)
                assert(load == nil)
                assert(loadfile == nil)
                assert(os == nil)
                assert(package == nil)
                assert(pcall == nil)
                assert(require == nil)
                assert(xpcall == nil)
                return {}
            end
        }
        "#,
    )
    .expect("write sandbox test module");

    let engine = LuaEngine::load(directory.path(), limits()).expect("module should load");
    let actions = engine
        .execute_command("sandbox", "sandbox", context())
        .await
        .expect("sandbox globals should remain unavailable");

    assert!(actions.is_empty());
}

#[test]
fn duplicate_commands_are_rejected() {
    let directory = TempDir::new().expect("temporary directory");
    for (file, module_id) in [("one.lua", "one"), ("two.lua", "two")] {
        std::fs::write(
            directory.path().join(file),
            format!(
                r#"
                return {{
                    manifest = {{
                        id = "{module_id}",
                        name = "{module_id}",
                        version = "1.0.0",
                        description = "Duplicate command test module.",
                        category = "test",
                        commands = {{
                            {{ name = "same", description = "Duplicate command." }}
                        }}
                    }},
                    on_command = function() return {{}} end
                }}
                "#
            ),
        )
        .expect("write test module");
    }

    let result = LuaEngine::load(directory.path(), limits());
    assert!(
        result.is_err(),
        "duplicate slash commands must fail startup"
    );
}

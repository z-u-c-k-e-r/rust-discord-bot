use serde_json::json;
use std::path::PathBuf;
use tempfile::tempdir;
use zuckerbot::lua::{LuaEngine, LuaLimits};

fn limits() -> LuaLimits {
    LuaLimits {
        memory_bytes: 4 * 1024 * 1024,
        instruction_limit: 200_000,
        hook_granularity: 100,
    }
}

fn engine() -> LuaEngine {
    let scripts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts");
    LuaEngine::load(scripts, limits())
        .expect("bundled Lua modules should load and schemas must be valid")
}

#[tokio::test]
async fn all_bundled_module_schemas_compile_and_load_successfully() {
    let engine = engine();
    let manifests = engine.manifests().await;
    assert!(
        !manifests.is_empty(),
        "bundled manifests should not be empty"
    );

    for manifest in manifests {
        assert!(
            manifest.config_schema.is_object(),
            "config schema for {} must be an object",
            manifest.id
        );
        // An empty config {} should be valid for modules where no properties are strictly required
        // or we test validate_module_config doesn't crash
        let _ = engine
            .validate_module_config(&manifest.id, &json!({}))
            .await;
    }
}

#[tokio::test]
async fn validate_module_config_checks_types_and_limits() {
    let engine = engine();

    // scheduler module defines minimum_delay_seconds as integer between 10 and 3600
    // Test 1: invalid type (string instead of integer)
    let invalid_type = json!({
        "minimum_delay_seconds": "not-an-integer"
    });
    let err_type = engine
        .validate_module_config("scheduler", &invalid_type)
        .await;
    assert!(
        err_type.is_err(),
        "string should be rejected for integer type"
    );
    let errors = err_type.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("integer") || e.contains("type")),
        "errors: {errors:?}"
    );

    // Test 2: below minimum (minimum_delay_seconds = 5 < 10)
    let below_min = json!({
        "minimum_delay_seconds": 5
    });
    let err_min = engine.validate_module_config("scheduler", &below_min).await;
    assert!(err_min.is_err(), "value below minimum should be rejected");
    let errors = err_min.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("less than the minimum") || e.contains("10")),
        "errors: {errors:?}"
    );

    // Test 3: above maximum (minimum_delay_seconds = 5000 > 3600)
    let above_max = json!({
        "minimum_delay_seconds": 5000
    });
    let err_max = engine.validate_module_config("scheduler", &above_max).await;
    assert!(err_max.is_err(), "value above maximum should be rejected");

    // Test 4: valid value (minimum_delay_seconds = 60)
    let valid = json!({
        "minimum_delay_seconds": 60,
        "user_reminders_enabled": true
    });
    let ok = engine.validate_module_config("scheduler", &valid).await;
    assert!(ok.is_ok(), "valid configuration should succeed: {ok:?}");
}

#[tokio::test]
async fn validate_module_config_rejects_additional_properties() {
    let engine = engine();

    // scheduler defines additionalProperties = false
    let extra_props = json!({
        "user_reminders_enabled": true,
        "unexpected_hacker_field": "injected"
    });
    let result = engine
        .validate_module_config("scheduler", &extra_props)
        .await;
    assert!(
        result.is_err(),
        "undeclared fields should be rejected when additionalProperties is false"
    );
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("additional") || e.contains("unexpected_hacker_field")),
        "errors: {errors:?}"
    );
}

#[tokio::test]
async fn validate_module_config_checks_enums_and_required_properties() {
    let dir = tempdir().expect("tempdir");
    let script_path = dir.path().join("test_schema.lua");

    let lua_code = r#"
return {
    manifest = {
        id = "custom_test",
        name = "Custom Test",
        version = "1.0.0",
        description = "Test module for enum and required validation",
        category = "Utility",
        default_enabled = true,
        commands = {},
        events = {},
        config_schema = {
            type = "object",
            required = { "mode", "threshold" },
            additionalProperties = false,
            properties = {
                mode = {
                    type = "string",
                    enum = { "fast", "strict", "lenient" }
                },
                threshold = {
                    type = "integer",
                    minimum = 1,
                    maximum = 100
                }
            }
        }
    }
}
"#;
    std::fs::write(&script_path, lua_code).expect("write script");

    let engine = LuaEngine::load(dir.path(), limits()).expect("load test engine");

    // Missing required field "threshold"
    let missing_required = json!({
        "mode": "fast"
    });
    let err_req = engine
        .validate_module_config("custom_test", &missing_required)
        .await;
    assert!(err_req.is_err(), "missing required property should fail");
    let errors = err_req.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("threshold") || e.contains("required")),
        "errors: {errors:?}"
    );

    // Invalid enum value
    let invalid_enum = json!({
        "mode": "invalid_mode",
        "threshold": 50
    });
    let err_enum = engine
        .validate_module_config("custom_test", &invalid_enum)
        .await;
    assert!(err_enum.is_err(), "invalid enum value should fail");
    let errors = err_enum.unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("enum")
            || e.contains("invalid_mode")
            || e.contains("is not one of")),
        "errors: {errors:?}"
    );

    // Perfectly valid payload
    let valid = json!({
        "mode": "strict",
        "threshold": 50
    });
    let ok = engine.validate_module_config("custom_test", &valid).await;
    assert!(ok.is_ok(), "valid payload should pass");
}

#[tokio::test]
async fn invalid_module_schema_at_startup_is_rejected() {
    let dir = tempdir().expect("tempdir");
    let script_path = dir.path().join("malformed_schema.lua");

    let lua_code = r#"
return {
    manifest = {
        id = "bad_schema",
        name = "Bad Schema",
        version = "1.0.0",
        description = "Module with malformed JSON schema",
        category = "Utility",
        default_enabled = true,
        commands = {},
        events = {},
        config_schema = {
            type = "unknown_json_schema_type_that_is_invalid"
        }
    }
}
"#;
    std::fs::write(&script_path, lua_code).expect("write script");

    let result = LuaEngine::load(dir.path(), limits());
    assert!(
        result.is_err(),
        "engine load must reject invalid JSON schemas at startup"
    );
}

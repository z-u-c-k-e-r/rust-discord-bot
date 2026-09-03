from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    if old not in content:
        raise SystemExit(f"expected source fragment is missing in {path}: {old[:120]!r}")
    file.write_text(content.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src/scheduler/mod.rs",
    "    pub const fn memory() -> Self {\n",
    "    pub fn memory() -> Self {\n",
)

replace_once(
    "src/lib.rs",
    "pub mod lua;\npub mod state;\n",
    "pub mod lua;\npub mod scheduler;\npub mod state;\n",
)

replace_once(
    "src/state.rs",
    "    lua::{LuaEngine, LuaLimits},\n    storage::Storage,\n",
    "    lua::{LuaEngine, LuaLimits},\n    scheduler::SchedulerStore,\n    storage::Storage,\n",
)
replace_once(
    "src/state.rs",
    "    pub scripts: LuaEngine,\n    pub storage: Storage,\n",
    "    pub scripts: LuaEngine,\n    pub scheduler: SchedulerStore,\n    pub storage: Storage,\n",
)
replace_once(
    "src/state.rs",
    "        let storage = Storage::connect(config.database_url.as_deref()).await?;\n        let user_agent",
    "        let storage = Storage::connect(config.database_url.as_deref()).await?;\n        let scheduler = SchedulerStore::connect(config.database_url.as_deref()).await?;\n        let user_agent",
)
replace_once(
    "src/state.rs",
    "            scripts,\n            storage,\n            http_client,\n",
    "            scripts,\n            scheduler,\n            storage,\n            http_client,\n",
)

replace_once(
    "src/lua/mod.rs",
    "mod progression;\n",
    "mod progression;\nmod scheduler;\n",
)
replace_once(
    "src/lua/mod.rs",
    "pub use progression::{ProgressionMetric, ProgressionOperation};\n",
    "pub use progression::{ProgressionMetric, ProgressionOperation};\npub use scheduler::{SchedulerOperation, SchedulerScope};\n",
)

replace_once(
    "src/lua/model.rs",
    "use super::progression::ProgressionOperation;\n",
    "use super::{progression::ProgressionOperation, scheduler::SchedulerOperation};\n",
)
replace_once(
    "src/lua/model.rs",
    "    Progression {\n        operation: ProgressionOperation,\n    },\n    Audit {\n",
    "    Progression {\n        operation: ProgressionOperation,\n    },\n    Scheduler {\n        operation: SchedulerOperation,\n    },\n    Audit {\n",
)
replace_once(
    "src/lua/model.rs",
    "            Self::Progression { operation } => operation.validate(),\n            Self::Audit { event, .. } => {\n",
    "            Self::Progression { operation } => operation.validate(),\n            Self::Scheduler { operation } => operation.validate(),\n            Self::Audit { event, .. } => {\n",
)

replace_once(
    "src/discord/mod.rs",
    "mod progression;\n",
    "mod progression;\nmod scheduler;\n",
)
replace_once(
    "src/discord/mod.rs",
    "use anyhow::Result;\n",
    "use std::sync::Arc;\n\nuse anyhow::Result;\n",
)
replace_once(
    "src/discord/mod.rs",
    "    client.start_autosharded().await?;\n    Ok(())\n",
    "    let scheduler_worker = tokio::spawn(crate::scheduler::run_worker(\n        Arc::clone(&client.http),\n        state.clone(),\n    ));\n    let client_result = client.start_autosharded().await;\n    scheduler_worker.abort();\n    client_result?;\n    Ok(())\n",
)

replace_once(
    "src/discord/executor.rs",
    "use super::{music, progression};\n",
    "use super::{music, progression, scheduler};\n",
)
replace_once(
    "src/discord/executor.rs",
    "        LuaAction::Audit { event, data } => {\n",
    "        LuaAction::Scheduler { operation } => {\n            scheduler::execute(\n                ctx,\n                state,\n                module_id,\n                scheduler::SchedulerExecutionContext::new(\n                    origin.guild_id,\n                    origin.channel_id,\n                    origin.actor_id,\n                    origin.actor_permissions,\n                    origin.enforce_actor_permissions,\n                ),\n                operation,\n            )\n            .await\n        }\n        LuaAction::Audit { event, data } => {\n",
)

replace_once(
    ".github/workflows/ci.yml",
    "      - feat/progression-economy\n",
    "      - feat/progression-economy\n      - feat/persistent-scheduler-reminders\n",
)

replace_once(
    "README.md",
    "- validated action API for replies, messages, moderation, roles, purge, music and audit events\n",
    "- validated action API for replies, messages, moderation, roles, purge, music, progression, persistent scheduling and audit events\n",
)
replace_once(
    "README.md",
    "| `automod.lua` | configurable `message_create` blocked-word example |\n",
    "| `automod.lua` | multi-rule message safety and anti-abuse engine |\n| `progression.lua` | XP, levels, coins, daily streaks, reputation and leaderboards |\n| `scheduler.lua` | persistent reminders and recurring server messages |\n| `community.lua` | suggestions, reports, feedback, applications and bug reports |\n| `utility.lua` | calculations, conversions, timestamps and Discord utilities |\n| `games.lua` | social games, dice, teams and cosmetic drops |\n| `staff_tools.lua` | announcements, broadcasts, alerts, rules and audit notes |\n| `server_info.lua` | rules, FAQ, links, support, staff and schedule |\n| `join_guard.lua` | account-age and suspicious-join safety signals |\n",
)
replace_once(
    "README.md",
    "- audit records for privileged actions and dashboard changes\n",
    "- audit records for privileged actions, scheduler transitions and dashboard changes\n- lease-based PostgreSQL scheduler with retries, pause/resume and an in-memory test fallback\n",
)

use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use serde_json::Value;

use super::GuildModuleSettings;

type ModuleKey = (String, String);

#[derive(Clone, Default)]
pub struct MemoryStore {
    settings: Arc<DashMap<ModuleKey, GuildModuleSettings>>,
    audit_log: Arc<DashMap<u64, Value>>,
    next_audit_id: Arc<std::sync::atomic::AtomicU64>,
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
}

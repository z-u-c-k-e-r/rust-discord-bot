use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicU64, Ordering},
};

use chrono::Utc;
use dashmap::DashMap;
use serde_json::Value;

use super::{GuildModuleSettings, ModerationCase};

type ModuleKey = (String, String);

#[derive(Clone, Default)]
pub struct MemoryStore {
    settings: Arc<DashMap<ModuleKey, GuildModuleSettings>>,
    audit_log: Arc<DashMap<u64, Value>>,
    next_audit_id: Arc<AtomicU64>,
    moderation_cases: Arc<DashMap<i64, ModerationCase>>,
    next_case_id: Arc<AtomicI64>,
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

    #[allow(clippy::too_many_arguments)]
    pub fn create_moderation_case(
        &self,
        guild_id: &str,
        target_user_id: &str,
        moderator_user_id: &str,
        action: &str,
        reason: &str,
        expires_at: Option<chrono::DateTime<Utc>>,
        metadata: Value,
        points: i32,
    ) -> ModerationCase {
        let id = self.next_case_id.fetch_add(1, Ordering::Relaxed) + 1;
        let moderation_case = ModerationCase {
            id,
            guild_id: guild_id.to_owned(),
            target_user_id: target_user_id.to_owned(),
            moderator_user_id: moderator_user_id.to_owned(),
            action: action.to_owned(),
            reason: Some(reason.to_owned()),
            expires_at,
            metadata,
            points,
            status: "open".to_owned(),
            resolution: None,
            resolved_by_user_id: None,
            resolved_at: None,
            created_at: Utc::now(),
        };
        self.moderation_cases.insert(id, moderation_case.clone());
        moderation_case
    }

    pub fn list_moderation_cases(
        &self,
        guild_id: &str,
        target_user_id: &str,
        include_resolved: bool,
        limit: u8,
    ) -> Vec<ModerationCase> {
        let mut cases = self
            .moderation_cases
            .iter()
            .filter(|entry| {
                let moderation_case = entry.value();
                moderation_case.guild_id == guild_id
                    && moderation_case.target_user_id == target_user_id
                    && (include_resolved || moderation_case.status == "open")
            })
            .map(|entry| entry.value().clone())
            .collect::<Vec<_>>();
        cases.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        cases.truncate(usize::from(limit));
        cases
    }

    pub fn resolve_moderation_case(
        &self,
        guild_id: &str,
        case_id: i64,
        resolved_by_user_id: &str,
        resolution: &str,
    ) -> Option<ModerationCase> {
        let mut moderation_case = self.moderation_cases.get_mut(&case_id)?;
        if moderation_case.guild_id != guild_id || moderation_case.status != "open" {
            return None;
        }

        moderation_case.status = "resolved".to_owned();
        moderation_case.resolution = Some(resolution.to_owned());
        moderation_case.resolved_by_user_id = Some(resolved_by_user_id.to_owned());
        moderation_case.resolved_at = Some(Utc::now());
        Some(moderation_case.value().clone())
    }

    pub fn active_moderation_points(&self, guild_id: &str, target_user_id: &str) -> i64 {
        let now = Utc::now();
        self.moderation_cases
            .iter()
            .filter(|entry| {
                let moderation_case = entry.value();
                moderation_case.guild_id == guild_id
                    && moderation_case.target_user_id == target_user_id
                    && moderation_case.status == "open"
                    && moderation_case
                        .expires_at
                        .as_ref()
                        .is_none_or(|expires_at| expires_at > &now)
            })
            .map(|entry| i64::from(entry.value().points))
            .sum()
    }

    pub fn record_audit(
        &self,
        guild_id: Option<&str>,
        actor_id: Option<&str>,
        module_id: &str,
        event: &str,
        data: Value,
    ) {
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
#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use serde_json::json;

    use super::MemoryStore;

    #[test]
    fn moderation_cases_are_listed_scored_and_resolved() {
        let store = MemoryStore::default();
        let first = store.create_moderation_case(
            "100000000000000001",
            "100000000000000002",
            "100000000000000003",
            "warning",
            "Spam",
            Some(Utc::now() + Duration::days(30)),
            json!({ "source": "test" }),
            2,
        );
        let second = store.create_moderation_case(
            "100000000000000001",
            "100000000000000002",
            "100000000000000003",
            "warning",
            "Repeated spam",
            None,
            json!({}),
            3,
        );

        assert_eq!(
            store.active_moderation_points("100000000000000001", "100000000000000002"),
            5
        );
        assert_eq!(
            store
                .list_moderation_cases("100000000000000001", "100000000000000002", false, 10,)
                .len(),
            2
        );

        let resolved = store
            .resolve_moderation_case(
                "100000000000000001",
                second.id,
                "100000000000000004",
                "Appeal accepted",
            )
            .expect("case should resolve");
        assert_eq!(resolved.status, "resolved");
        assert_eq!(
            store.active_moderation_points("100000000000000001", "100000000000000002"),
            2
        );
        assert_eq!(first.id, 1);
    }
}

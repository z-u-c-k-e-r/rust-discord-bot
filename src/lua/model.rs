use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaModuleManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub category: String,
    #[serde(default = "default_true")]
    pub default_enabled: bool,
    #[serde(default)]
    pub commands: Vec<LuaCommandDefinition>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "empty_object")]
    pub config_schema: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaCommandDefinition {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub options: Vec<LuaCommandOption>,
    #[serde(default)]
    pub default_member_permissions: Option<String>,
    #[serde(default)]
    pub integration_types: Option<Vec<LuaInstallationContext>>,
    #[serde(default)]
    pub contexts: Option<Vec<LuaInteractionContext>>,
    #[serde(default)]
    pub nsfw: bool,
    /// Compatibility field for pre-context modules. New modules must use `contexts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_permission: Option<bool>,
}

impl LuaCommandDefinition {
    pub fn resolved_integration_types(&self) -> Vec<LuaInstallationContext> {
        self.integration_types
            .clone()
            .unwrap_or_else(|| vec![LuaInstallationContext::Guild])
    }

    pub fn resolved_contexts(&self) -> Vec<LuaInteractionContext> {
        self.contexts.clone().unwrap_or_else(|| {
            if self.dm_permission == Some(true) {
                vec![LuaInteractionContext::Guild, LuaInteractionContext::BotDm]
            } else {
                vec![LuaInteractionContext::Guild]
            }
        })
    }

    pub fn validate_contexts(&self) -> Result<(), String> {
        if self.contexts.is_some() && self.dm_permission.is_some() {
            return Err(
                "contexts and the deprecated dm_permission field cannot be used together"
                    .to_owned(),
            );
        }

        let integration_types = self.resolved_integration_types();
        if integration_types.is_empty() {
            return Err("integration_types cannot be empty".to_owned());
        }
        if has_duplicates(&integration_types) {
            return Err("integration_types cannot contain duplicates".to_owned());
        }

        let contexts = self.resolved_contexts();
        if contexts.is_empty() {
            return Err("contexts cannot be empty".to_owned());
        }
        if has_duplicates(&contexts) {
            return Err("contexts cannot contain duplicates".to_owned());
        }
        if contexts.contains(&LuaInteractionContext::PrivateChannel)
            && !integration_types.contains(&LuaInstallationContext::User)
        {
            return Err("private_channel context requires the user installation type".to_owned());
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuaInstallationContext {
    Guild,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuaInteractionContext {
    Guild,
    BotDm,
    PrivateChannel,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaCommandOption {
    #[serde(rename = "type")]
    pub kind: LuaOptionKind,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub autocomplete: bool,
    #[serde(default)]
    pub choices: Vec<LuaCommandChoice>,
    #[serde(default)]
    pub options: Vec<Self>,
    #[serde(default)]
    pub min_value: Option<f64>,
    #[serde(default)]
    pub max_value: Option<f64>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaCommandChoice {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LuaOptionKind {
    String,
    Integer,
    Number,
    Boolean,
    User,
    Channel,
    Role,
    Mentionable,
    Attachment,
    Subcommand,
    SubcommandGroup,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaExecutionContext {
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub user_id: String,
    pub user_name: String,
    pub member_roles: Vec<String>,
    pub member_permissions: String,
    pub locale: String,
    #[serde(default)]
    pub options: Value,
    #[serde(default = "empty_object")]
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LuaEventContext {
    pub name: String,
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub actor_id: Option<String>,
    #[serde(default = "empty_object")]
    pub data: Value,
    #[serde(default = "empty_object")]
    pub config: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LuaAction {
    Reply {
        content: String,
        #[serde(default)]
        ephemeral: bool,
    },
    SendMessage {
        #[serde(default)]
        channel_id: Option<String>,
        content: String,
    },
    DeleteMessage {
        channel_id: String,
        message_id: String,
    },
    TimeoutMember {
        user_id: String,
        seconds: u64,
        #[serde(default)]
        reason: Option<String>,
    },
    KickMember {
        user_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
    BanMember {
        user_id: String,
        #[serde(default)]
        delete_message_days: u8,
        #[serde(default)]
        reason: Option<String>,
    },
    AddRole {
        user_id: String,
        role_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
    RemoveRole {
        user_id: String,
        role_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
    Purge {
        #[serde(default)]
        channel_id: Option<String>,
        amount: u8,
    },
    Music {
        operation: MusicOperation,
        #[serde(default)]
        query: Option<String>,
    },
    CreateModerationCase {
        target_user_id: String,
        case_type: String,
        reason: String,
        #[serde(default)]
        points: u16,
        #[serde(default)]
        expires_in_seconds: Option<u64>,
        #[serde(default)]
        metadata: BTreeMap<String, Value>,
        #[serde(default)]
        escalation_rules: Vec<ModerationEscalationRule>,
    },
    ListModerationCases {
        target_user_id: String,
        #[serde(default = "default_case_limit")]
        limit: u8,
        #[serde(default)]
        include_resolved: bool,
    },
    ResolveModerationCase {
        case_id: i64,
        resolution: String,
    },
    Audit {
        event: String,
        #[serde(default)]
        data: BTreeMap<String, Value>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModerationEscalationRule {
    pub threshold_points: u16,
    pub action: ModerationEscalationAction,
    #[serde(default)]
    pub duration_seconds: Option<u64>,
    #[serde(default)]
    pub delete_message_days: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationEscalationAction {
    Timeout,
    Kick,
    Ban,
}

impl LuaAction {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Reply { content, .. } | Self::SendMessage { content, .. } => {
                validate_content(content)
            }
            Self::DeleteMessage {
                channel_id,
                message_id,
            } => {
                validate_snowflake(channel_id)?;
                validate_snowflake(message_id)
            }
            Self::TimeoutMember {
                user_id,
                seconds,
                reason,
            } => {
                validate_snowflake(user_id)?;
                if *seconds == 0 || *seconds > 2_419_200 {
                    return Err(
                        "timeout seconds must be between 1 and 2419200 (28 days)".to_owned()
                    );
                }
                validate_reason(reason)
            }
            Self::KickMember { user_id, reason } => {
                validate_snowflake(user_id)?;
                validate_reason(reason)
            }
            Self::BanMember {
                user_id,
                delete_message_days,
                reason,
            } => {
                validate_snowflake(user_id)?;
                if *delete_message_days > 7 {
                    return Err("delete_message_days cannot exceed 7".to_owned());
                }
                validate_reason(reason)
            }
            Self::AddRole {
                user_id,
                role_id,
                reason,
            }
            | Self::RemoveRole {
                user_id,
                role_id,
                reason,
            } => {
                validate_snowflake(user_id)?;
                validate_snowflake(role_id)?;
                validate_reason(reason)
            }
            Self::Purge { channel_id, amount } => {
                if let Some(channel_id) = channel_id {
                    validate_snowflake(channel_id)?;
                }
                if !(1..=100).contains(amount) {
                    return Err("purge amount must be between 1 and 100".to_owned());
                }
                Ok(())
            }
            Self::CreateModerationCase {
                target_user_id,
                case_type,
                reason,
                points,
                expires_in_seconds,
                metadata,
                escalation_rules,
            } => {
                validate_snowflake(target_user_id)?;
                validate_case_type(case_type)?;
                validate_required_text(reason, "moderation case reason", 512)?;
                if *points > 1_000 {
                    return Err("moderation case points cannot exceed 1000".to_owned());
                }
                if expires_in_seconds.is_some_and(|seconds| seconds == 0 || seconds > 31_536_000) {
                    return Err(
                        "moderation case expiry must be between 1 second and 365 days".to_owned(),
                    );
                }
                if metadata.len() > 32 {
                    return Err("moderation case metadata cannot exceed 32 keys".to_owned());
                }
                let metadata_size = serde_json::to_vec(metadata)
                    .map_err(|error| format!("invalid moderation case metadata: {error}"))?
                    .len();
                if metadata_size > 16 * 1024 {
                    return Err("moderation case metadata cannot exceed 16 KiB".to_owned());
                }
                if escalation_rules.len() > 10 {
                    return Err("moderation escalation cannot exceed 10 rules".to_owned());
                }
                if !escalation_rules.is_empty() && *points == 0 {
                    return Err("moderation escalation requires a positive point value".to_owned());
                }

                let mut previous_threshold = 0;
                for rule in escalation_rules {
                    if rule.threshold_points == 0 || rule.threshold_points > 1_000 {
                        return Err(
                            "escalation thresholds must be between 1 and 1000 points".to_owned()
                        );
                    }
                    if rule.threshold_points <= previous_threshold {
                        return Err("escalation thresholds must be strictly increasing".to_owned());
                    }
                    previous_threshold = rule.threshold_points;

                    match rule.action {
                        ModerationEscalationAction::Timeout => {
                            if rule
                                .duration_seconds
                                .is_none_or(|seconds| seconds == 0 || seconds > 2_419_200)
                            {
                                return Err(
                                    "timeout escalation requires 1 to 2419200 seconds".to_owned()
                                );
                            }
                            if rule.delete_message_days != 0 {
                                return Err(
                                    "timeout escalation cannot delete message history".to_owned()
                                );
                            }
                        }
                        ModerationEscalationAction::Kick => {
                            if rule.duration_seconds.is_some() || rule.delete_message_days != 0 {
                                return Err(
                                    "kick escalation cannot define duration or message deletion"
                                        .to_owned(),
                                );
                            }
                        }
                        ModerationEscalationAction::Ban => {
                            if rule.duration_seconds.is_some() || rule.delete_message_days > 7 {
                                return Err(
                                    "ban escalation accepts only 0 to 7 delete_message_days"
                                        .to_owned(),
                                );
                            }
                        }
                    }
                }
                Ok(())
            }
            Self::ListModerationCases {
                target_user_id,
                limit,
                ..
            } => {
                validate_snowflake(target_user_id)?;
                if !(1..=25).contains(limit) {
                    return Err("moderation case list limit must be between 1 and 25".to_owned());
                }
                Ok(())
            }
            Self::ResolveModerationCase {
                case_id,
                resolution,
            } => {
                if *case_id <= 0 {
                    return Err("moderation case id must be greater than zero".to_owned());
                }
                validate_required_text(resolution, "moderation case resolution", 512)
            }
            Self::Music { query, operation } => {
                if matches!(operation, MusicOperation::Play)
                    && query.as_deref().is_none_or(str::is_empty)
                {
                    return Err("music play requires a query".to_owned());
                }
                Ok(())
            }
            Self::Audit { event, .. } => {
                if event.trim().is_empty() || event.len() > 64 {
                    return Err("audit event must contain 1 to 64 bytes".to_owned());
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicOperation {
    Play,
    Pause,
    Resume,
    Skip,
    Stop,
    Leave,
    Queue,
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}

fn validate_content(content: &str) -> Result<(), String> {
    let length = content.chars().count();
    if length == 0 || length > 2_000 {
        return Err("message content must contain 1 to 2000 characters".to_owned());
    }
    Ok(())
}

fn validate_reason(reason: &Option<String>) -> Result<(), String> {
    if reason
        .as_ref()
        .is_some_and(|value| value.chars().count() > 512)
    {
        return Err("audit reason cannot exceed 512 characters".to_owned());
    }
    Ok(())
}

fn validate_case_type(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        return Err(
            "moderation case type must contain 1 to 32 lowercase ASCII characters".to_owned(),
        );
    }
    Ok(())
}

fn validate_required_text(value: &str, field: &str, max_characters: usize) -> Result<(), String> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > max_characters {
        return Err(format!(
            "{field} must contain 1 to {max_characters} characters"
        ));
    }
    Ok(())
}

fn validate_snowflake(value: &str) -> Result<(), String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{value:?} is not a Discord snowflake"))?;
    if parsed == 0 {
        return Err("Discord snowflakes must be greater than zero".to_owned());
    }
    Ok(())
}

const fn default_true() -> bool {
    true
}

const fn default_case_limit() -> u8 {
    10
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{LuaCommandDefinition, LuaInstallationContext, LuaInteractionContext};

    fn command(value: serde_json::Value) -> LuaCommandDefinition {
        serde_json::from_value(value).expect("command definition should deserialize")
    }

    #[test]
    fn command_contexts_default_to_guild_only() {
        let definition = command(json!({
            "name": "hello",
            "description": "Says hello."
        }));

        assert_eq!(
            definition.resolved_integration_types(),
            vec![LuaInstallationContext::Guild]
        );
        assert_eq!(
            definition.resolved_contexts(),
            vec![LuaInteractionContext::Guild]
        );
        assert!(definition.validate_contexts().is_ok());
    }

    #[test]
    fn legacy_dm_permission_maps_without_serializing_the_deprecated_field() {
        let definition = command(json!({
            "name": "hello",
            "description": "Says hello.",
            "dm_permission": true
        }));

        assert_eq!(
            definition.resolved_contexts(),
            vec![LuaInteractionContext::Guild, LuaInteractionContext::BotDm]
        );
        assert!(definition.validate_contexts().is_ok());
    }

    #[test]
    fn private_channel_requires_user_installation() {
        let definition = command(json!({
            "name": "hello",
            "description": "Says hello.",
            "integration_types": ["guild"],
            "contexts": ["private_channel"]
        }));

        assert_eq!(
            definition.validate_contexts(),
            Err("private_channel context requires the user installation type".to_owned())
        );
    }

    #[test]
    fn contexts_cannot_be_mixed_with_legacy_dm_permission() {
        let definition = command(json!({
            "name": "hello",
            "description": "Says hello.",
            "contexts": ["guild"],
            "dm_permission": false
        }));

        assert!(definition.validate_contexts().is_err());
    }
}

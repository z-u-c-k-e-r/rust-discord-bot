use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub commands: Vec<CommandManifest>,
}

fn default_version() -> String {
    "0.1.0".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandManifest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub options: Vec<CommandOptionManifest>,
    #[serde(default)]
    pub required_permissions: Vec<String>,
    #[serde(default = "default_true")]
    pub dm_permission: bool,
    #[serde(default)]
    pub nsfw: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOptionManifest {
    pub name: String,
    pub description: String,
    pub kind: CommandOptionKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub min_integer: Option<i64>,
    #[serde(default)]
    pub max_integer: Option<i64>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOptionKind {
    String,
    Integer,
    Number,
    Boolean,
    User,
    Channel,
    Role,
    Mentionable,
    Attachment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContext {
    pub command: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub user_id: String,
    pub username: String,
    #[serde(default)]
    pub options: HashMap<String, Value>,
    #[serde(default)]
    pub module_config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LuaAction {
    Reply {
        content: String,
        #[serde(default)]
        ephemeral: bool,
    },
    SendMessage {
        content: String,
    },
    Kick {
        user_id: String,
        #[serde(default = "default_reason")]
        reason: String,
    },
    Ban {
        user_id: String,
        #[serde(default = "default_reason")]
        reason: String,
        #[serde(default)]
        delete_message_seconds: u32,
    },
    VoiceJoin,
    VoiceLeave,
    MusicPlay {
        query: String,
    },
    MusicPause,
    MusicResume,
    MusicSkip,
    MusicStop,
}

fn default_reason() -> String {
    "No reason supplied".to_owned()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildConfig {
    /// `None` means that every installed module is enabled. `Some` is an explicit allow-list.
    #[serde(default)]
    pub enabled_modules: Option<BTreeSet<String>>,
    #[serde(default)]
    pub module_config: BTreeMap<String, Value>,
}

impl GuildConfig {
    pub fn module_enabled(&self, module: &str) -> bool {
        self.enabled_modules
            .as_ref()
            .is_none_or(|modules| modules.contains(module))
    }

    pub fn config_for(&self, module: &str) -> Value {
        self.module_config
            .get(module)
            .cloned()
            .unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<String>,
}

impl From<&ModuleManifest> for ModuleSummary {
    fn from(manifest: &ModuleManifest) -> Self {
        Self {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            commands: manifest
                .commands
                .iter()
                .map(|command| command.name.clone())
                .collect(),
        }
    }
}

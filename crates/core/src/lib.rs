use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const DISCORD_MAX_MESSAGE_CODEPOINTS: usize = 2_000;
pub const DISCORD_MAX_COMMAND_NAME_LENGTH: usize = 32;
pub const DISCORD_MAX_COMMAND_DESCRIPTION_LENGTH: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub dm_permission: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandContext {
    pub command_name: String,
    pub interaction_id: String,
    pub user_id: String,
    pub user_name: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub locale: String,
    #[serde(default)]
    pub options: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResponse {
    pub content: String,
    #[serde(default)]
    pub ephemeral: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("command name must contain between 1 and {DISCORD_MAX_COMMAND_NAME_LENGTH} characters")]
    InvalidCommandNameLength,
    #[error("command name may contain only lowercase letters, digits, '-' and '_'")]
    InvalidCommandNameCharacters,
    #[error(
        "command description must contain between 1 and {DISCORD_MAX_COMMAND_DESCRIPTION_LENGTH} characters"
    )]
    InvalidCommandDescription,
    #[error("response content exceeds Discord's {DISCORD_MAX_MESSAGE_CODEPOINTS}-character limit")]
    ResponseTooLong,
    #[error("response content may not be empty")]
    EmptyResponse,
}

impl CommandSpec {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let name_length = self.name.chars().count();
        if !(1..=DISCORD_MAX_COMMAND_NAME_LENGTH).contains(&name_length) {
            return Err(ValidationError::InvalidCommandNameLength);
        }

        if !self
            .name
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '-' | '_'))
        {
            return Err(ValidationError::InvalidCommandNameCharacters);
        }

        let description_length = self.description.chars().count();
        if !(1..=DISCORD_MAX_COMMAND_DESCRIPTION_LENGTH).contains(&description_length) {
            return Err(ValidationError::InvalidCommandDescription);
        }

        Ok(())
    }
}

impl CommandResponse {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.content.is_empty() {
            return Err(ValidationError::EmptyResponse);
        }

        if self.content.chars().count() > DISCORD_MAX_MESSAGE_CODEPOINTS {
            return Err(ValidationError::ResponseTooLong);
        }

        Ok(())
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_command_specification() {
        let command = CommandSpec {
            name: "server-info".to_owned(),
            description: "Shows information about the server.".to_owned(),
            dm_permission: false,
        };

        assert_eq!(command.validate(), Ok(()));
    }

    #[test]
    fn rejects_uppercase_command_names() {
        let command = CommandSpec {
            name: "Ping".to_owned(),
            description: "Checks whether the bot is online.".to_owned(),
            dm_permission: true,
        };

        assert_eq!(
            command.validate(),
            Err(ValidationError::InvalidCommandNameCharacters)
        );
    }

    #[test]
    fn rejects_responses_over_discord_limit() {
        let response = CommandResponse {
            content: "x".repeat(DISCORD_MAX_MESSAGE_CODEPOINTS + 1),
            ephemeral: false,
        };

        assert_eq!(response.validate(), Err(ValidationError::ResponseTooLong));
    }
}

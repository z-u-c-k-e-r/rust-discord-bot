use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug)]
pub struct BotConfig {
    discord_token: String,
    pub development_guild_id: Option<u64>,
    pub plugin_directory: PathBuf,
    pub lua_memory_limit_bytes: usize,
    pub lua_instruction_limit: u64,
    pub enable_message_content: bool,
    pub enable_guild_members: bool,
    pub enable_guild_presences: bool,
}

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
}

impl BotConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let discord_token = required("DISCORD_TOKEN")?;
        let development_guild_id = optional("DISCORD_DEVELOPMENT_GUILD_ID")
            .map(|value| parse::<u64>("DISCORD_DEVELOPMENT_GUILD_ID", &value))
            .transpose()?;

        Ok(Self {
            discord_token,
            development_guild_id,
            plugin_directory: PathBuf::from(
                optional("PLUGIN_DIRECTORY").unwrap_or_else(|| "plugins".to_owned()),
            ),
            lua_memory_limit_bytes: parse_or_default(
                "LUA_MEMORY_LIMIT_BYTES",
                16 * 1024 * 1024,
            )?,
            lua_instruction_limit: parse_or_default("LUA_INSTRUCTION_LIMIT", 1_000_000)?,
            enable_message_content: parse_bool_or_default(
                "DISCORD_ENABLE_MESSAGE_CONTENT",
                false,
            )?,
            enable_guild_members: parse_bool_or_default(
                "DISCORD_ENABLE_GUILD_MEMBERS",
                false,
            )?,
            enable_guild_presences: parse_bool_or_default(
                "DISCORD_ENABLE_GUILD_PRESENCES",
                false,
            )?,
        })
    }

    pub fn discord_token(&self) -> &str {
        &self.discord_token
    }
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let address = optional("API_BIND_ADDRESS").unwrap_or_else(|| "0.0.0.0:8080".to_owned());
        Ok(Self {
            bind_address: parse("API_BIND_ADDRESS", &address)?,
        })
    }
}

fn required(name: &str) -> Result<String> {
    optional(name).with_context(|| format!("environment variable {name} is required"))
}

fn optional(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse<T>(name: &str, value: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .parse::<T>()
        .with_context(|| format!("environment variable {name} has an invalid value"))
}

fn parse_or_default<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    optional(name)
        .map(|value| parse(name, &value))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_bool_or_default(name: &str, default: bool) -> Result<bool> {
    let Some(value) = optional(name) else {
        return Ok(default);
    };

    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("environment variable {name} must be true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_boolean_spellings() {
        for value in ["true", "TRUE", "1", "yes", "on"] {
            unsafe { env::set_var("ZUCKERBOT_TEST_BOOL", value) };
            assert!(parse_bool_or_default("ZUCKERBOT_TEST_BOOL", false).unwrap());
        }

        unsafe { env::remove_var("ZUCKERBOT_TEST_BOOL") };
    }
}

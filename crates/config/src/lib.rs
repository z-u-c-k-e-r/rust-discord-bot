use std::{
    env, fmt,
    net::SocketAddr,
    path::PathBuf,
    time::Duration,
};

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

#[derive(Clone)]
pub struct ApiConfig {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub redis_url: String,
    pub postgres_max_connections: u32,
    pub dependency_timeout: Duration,
    pub migration_timeout: Duration,
    pub run_migrations: bool,
    control_plane_dev_token: Option<String>,
    pub control_plane_dev_actor_id: Option<String>,
}

impl fmt::Debug for ApiConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiConfig")
            .field("bind_address", &self.bind_address)
            .field("database_url", &"[redacted]")
            .field("redis_url", &"[redacted]")
            .field("postgres_max_connections", &self.postgres_max_connections)
            .field("dependency_timeout", &self.dependency_timeout)
            .field("migration_timeout", &self.migration_timeout)
            .field("run_migrations", &self.run_migrations)
            .field(
                "control_plane_dev_token",
                &self.control_plane_dev_token.as_ref().map(|_| "[redacted]"),
            )
            .field(
                "control_plane_dev_actor_id",
                &self.control_plane_dev_actor_id,
            )
            .finish()
    }
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
            lua_memory_limit_bytes: parse_or_default("LUA_MEMORY_LIMIT_BYTES", 16 * 1024 * 1024)?,
            lua_instruction_limit: parse_or_default("LUA_INSTRUCTION_LIMIT", 1_000_000)?,
            enable_message_content: parse_bool_or_default("DISCORD_ENABLE_MESSAGE_CONTENT", false)?,
            enable_guild_members: parse_bool_or_default("DISCORD_ENABLE_GUILD_MEMBERS", false)?,
            enable_guild_presences: parse_bool_or_default("DISCORD_ENABLE_GUILD_PRESENCES", false)?,
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
        let control_plane_dev_token = optional("CONTROL_PLANE_DEV_TOKEN");
        let control_plane_dev_actor_id = optional("CONTROL_PLANE_DEV_ACTOR_ID");

        match (&control_plane_dev_token, &control_plane_dev_actor_id) {
            (Some(token), Some(actor_id)) => {
                if token.len() < 32 {
                    bail!("CONTROL_PLANE_DEV_TOKEN must contain at least 32 characters");
                }
                if !is_snowflake(actor_id) {
                    bail!("CONTROL_PLANE_DEV_ACTOR_ID must be a Discord snowflake");
                }
            }
            (None, None) => {}
            _ => {
                bail!(
                    "CONTROL_PLANE_DEV_TOKEN and CONTROL_PLANE_DEV_ACTOR_ID must be configured together"
                );
            }
        }

        Ok(Self {
            bind_address: parse("API_BIND_ADDRESS", &address)?,
            database_url: optional("DATABASE_URL").unwrap_or_else(|| {
                "postgres://zuckerbot:zuckerbot@localhost:5432/zuckerbot".to_owned()
            }),
            redis_url: optional("REDIS_URL")
                .unwrap_or_else(|| "redis://localhost:6379".to_owned()),
            postgres_max_connections: parse_or_default("POSTGRES_MAX_CONNECTIONS", 10)?,
            dependency_timeout: Duration::from_millis(parse_or_default(
                "DEPENDENCY_TIMEOUT_MS",
                2_000_u64,
            )?),
            migration_timeout: Duration::from_secs(parse_or_default(
                "MIGRATION_TIMEOUT_SECONDS",
                30_u64,
            )?),
            run_migrations: parse_bool_or_default("API_RUN_MIGRATIONS", true)?,
            control_plane_dev_token,
            control_plane_dev_actor_id,
        })
    }

    pub fn control_plane_dev_token(&self) -> Option<&str> {
        self.control_plane_dev_token.as_deref()
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

fn is_snowflake(value: &str) -> bool {
    (1..=20).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit())
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

    #[test]
    fn validates_discord_snowflakes_without_numeric_conversion() {
        assert!(is_snowflake("123456789012345678"));
        assert!(!is_snowflake(""));
        assert!(!is_snowflake("123abc"));
        assert!(!is_snowflake("123456789012345678901"));
    }
}

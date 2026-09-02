use std::{
    collections::HashSet,
    env,
    net::SocketAddr,
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};

#[derive(Clone, Debug)]
pub struct Config {
    pub discord_token: String,
    pub discord_application_id: u64,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub discord_oauth_redirect_url: String,
    pub discord_dev_guild_id: Option<u64>,
    pub dashboard_bind: SocketAddr,
    pub dashboard_public_url: String,
    pub database_url: Option<String>,
    pub scripts_dir: PathBuf,
    pub session_cookie_secure: bool,
    pub session_ttl_seconds: i64,
    pub lua_memory_limit_bytes: usize,
    pub lua_instruction_limit: i64,
    pub lua_hook_granularity: u32,
    pub music_allowed_hosts: HashSet<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let discord_application_id = required("DISCORD_APPLICATION_ID")?
            .parse()
            .context("DISCORD_APPLICATION_ID must be an unsigned integer")?;

        let discord_client_id =
            env::var("DISCORD_CLIENT_ID").unwrap_or_else(|_| discord_application_id.to_string());

        let dashboard_bind = env::var("DASHBOARD_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_owned())
            .parse()
            .context("DASHBOARD_BIND must be in host:port format")?;

        let music_allowed_hosts = env::var("MUSIC_ALLOWED_HOSTS")
            .unwrap_or_else(|_| {
                [
                    "youtube.com",
                    "www.youtube.com",
                    "m.youtube.com",
                    "youtu.be",
                    "soundcloud.com",
                    "www.soundcloud.com",
                    "bandcamp.com",
                ]
                .join(",")
            })
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();

        let config = Self {
            discord_token: required("DISCORD_TOKEN")?,
            discord_application_id,
            discord_client_id,
            discord_client_secret: required("DISCORD_CLIENT_SECRET")?,
            discord_oauth_redirect_url: env::var("DISCORD_OAUTH_REDIRECT_URL")
                .unwrap_or_else(|_| {
                    "http://127.0.0.1:8080/auth/discord/callback".to_owned()
                }),
            discord_dev_guild_id: optional_parse("DISCORD_DEV_GUILD_ID")?,
            dashboard_bind,
            dashboard_public_url: env::var("DASHBOARD_PUBLIC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned()),
            database_url: optional_non_empty("DATABASE_URL"),
            scripts_dir: env::var("SCRIPTS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("scripts")),
            session_cookie_secure: env_bool("SESSION_COOKIE_SECURE", false)?,
            session_ttl_seconds: env_parse("SESSION_TTL_SECONDS", 604_800_i64)?,
            lua_memory_limit_bytes: env_parse(
                "LUA_MEMORY_LIMIT_BYTES",
                8 * 1024 * 1024_usize,
            )?,
            lua_instruction_limit: env_parse("LUA_INSTRUCTION_LIMIT", 500_000_i64)?,
            lua_hook_granularity: env_parse("LUA_HOOK_GRANULARITY", 1_000_u32)?,
            music_allowed_hosts,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.session_ttl_seconds <= 0 {
            return Err(anyhow!("SESSION_TTL_SECONDS must be greater than zero"));
        }
        if self.lua_memory_limit_bytes < 256 * 1024 {
            return Err(anyhow!(
                "LUA_MEMORY_LIMIT_BYTES must be at least 262144 bytes"
            ));
        }
        if self.lua_instruction_limit <= 0 {
            return Err(anyhow!("LUA_INSTRUCTION_LIMIT must be greater than zero"));
        }
        if self.lua_hook_granularity == 0 {
            return Err(anyhow!("LUA_HOOK_GRANULARITY must be greater than zero"));
        }
        if self.music_allowed_hosts.is_empty() {
            return Err(anyhow!("MUSIC_ALLOWED_HOSTS cannot be empty"));
        }

        Ok(())
    }
}

fn required(name: &str) -> Result<String> {
    env::var(name)
        .with_context(|| format!("{name} is required"))
        .and_then(|value| {
            if value.trim().is_empty() {
                Err(anyhow!("{name} cannot be empty"))
            } else {
                Ok(value)
            }
        })
}

fn optional_non_empty(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn optional_parse<T>(name: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    optional_non_empty(name)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("{name} contains an invalid value"))
        })
        .transpose()
}

fn env_parse<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match optional_non_empty(name) {
        Some(value) => value
            .parse()
            .with_context(|| format!("{name} contains an invalid value")),
        None => Ok(default),
    }
}

fn env_bool(name: &str, default: bool) -> Result<bool> {
    match optional_non_empty(name) {
        None => Ok(default),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(anyhow!(
                "{name} must be one of: true, false, 1, 0, yes, no, on, off"
            )),
        },
    }
}

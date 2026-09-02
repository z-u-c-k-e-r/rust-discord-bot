use std::{
    env,
    net::SocketAddr,
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serenity::all::GatewayIntents;

#[derive(Debug, Clone)]
pub struct Config {
    pub discord_token: String,
    pub web_bind: SocketAddr,
    pub dashboard_token: String,
    pub scripts_dir: PathBuf,
    pub data_dir: PathBuf,
    pub lua_limits: LuaLimits,
}

#[derive(Debug, Clone, Copy)]
pub struct LuaLimits {
    pub memory_bytes: usize,
    pub instruction_limit: u64,
    pub timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let discord_token = required("DISCORD_TOKEN")?;
        let dashboard_token = required("DASHBOARD_TOKEN")?;
        if dashboard_token.len() < 24 {
            bail!("DASHBOARD_TOKEN must contain at least 24 characters");
        }

        let web_bind = value("WEB_BIND", "0.0.0.0:8080")
            .parse()
            .context("WEB_BIND must be a valid socket address")?;

        let memory_bytes = parse("LUA_MEMORY_BYTES", 8 * 1024 * 1024)?;
        let instruction_limit = parse("LUA_INSTRUCTION_LIMIT", 250_000)?;
        let timeout_ms = parse("LUA_TIMEOUT_MS", 100_u64)?;

        Ok(Self {
            discord_token,
            web_bind,
            dashboard_token,
            scripts_dir: PathBuf::from(value("SCRIPTS_DIR", "scripts")),
            data_dir: PathBuf::from(value("DATA_DIR", "data")),
            lua_limits: LuaLimits {
                memory_bytes,
                instruction_limit,
                timeout: Duration::from_millis(timeout_ms),
            },
        })
    }

    pub fn gateway_intents(&self) -> GatewayIntents {
        GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::GUILD_VOICE_STATES
            | GatewayIntents::MESSAGE_CONTENT
    }
}

fn required(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("missing required environment variable {key}"))
}

fn value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + ToString,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    env::var(key)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .with_context(|| format!("{key} has an invalid value"))
}

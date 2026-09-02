use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::Config,
    lua::{LuaEngine, LuaLimits},
    storage::Storage,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub scripts: LuaEngine,
    pub storage: Storage,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub async fn bootstrap(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let limits = LuaLimits {
            memory_bytes: config.lua_memory_limit_bytes,
            instruction_limit: config.lua_instruction_limit,
            hook_granularity: config.lua_hook_granularity,
        };
        let scripts = LuaEngine::load(&config.scripts_dir, limits)?;
        let storage = Storage::connect(config.database_url.as_deref()).await?;
        let http_client = reqwest::Client::builder()
            .user_agent(concat!("ZuckerBot/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            config,
            scripts,
            storage,
            http_client,
        })
    }
}

use std::sync::Arc;

use anyhow::Result;

use crate::{
    config::Config,
    lua::{LuaEngine, LuaLimits},
    moderation_cases::ModerationCaseStore,
    scheduler::SchedulerStore,
    session::SessionStore,
    storage::Storage,
    tickets::TicketStore,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub scripts: LuaEngine,
    pub scheduler: SchedulerStore,
    pub storage: Storage,
    pub tickets: TicketStore,
    pub moderation_cases: ModerationCaseStore,
    pub http_client: reqwest::Client,
    pub media_http_client: reqwest_songbird::Client,
    pub metrics: Arc<crate::metrics::Metrics>,
    pub sessions: SessionStore,
    pub rate_limiter: Arc<crate::rate_limit::RateLimiter>,
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
        let scheduler = SchedulerStore::connect(config.database_url.as_deref()).await?;
        let tickets = TicketStore::connect(config.database_url.as_deref()).await?;
        let moderation_cases = ModerationCaseStore::connect(config.database_url.as_deref()).await?;
        let sessions = SessionStore::connect(config.database_url.as_deref()).await?;
        let user_agent = concat!("ZuckerBot/", env!("CARGO_PKG_VERSION"));
        let http_client = reqwest::Client::builder().user_agent(user_agent).build()?;
        let media_http_client = reqwest_songbird::Client::builder()
            .user_agent(user_agent)
            .redirect(reqwest_songbird::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 {
                    return attempt.error("zbyt wiele przekierowań w odtwarzaczu mediów");
                }

                let next_url = attempt.url();
                if next_url.scheme() != "https" {
                    return attempt
                        .error("przekierowanie w odtwarzaczu mediów musi używać protokołu HTTPS");
                }

                if let Some(host) = next_url.host_str()
                    && let Ok(ip) = host.parse::<std::net::IpAddr>()
                    && crate::discord::music::is_blocked_ip(ip)
                {
                    return attempt.error("przekierowanie do zablokowanego prywatnego adresu IP");
                }

                attempt.follow()
            }))
            .build()?;

        let metrics = Arc::new(crate::metrics::Metrics::new());
        let rate_limiter = Arc::new(crate::rate_limit::RateLimiter::new());
        Ok(Self {
            config,
            scripts,
            scheduler,
            storage,
            tickets,
            moderation_cases,
            http_client,
            media_http_client,
            metrics,
            sessions,
            rate_limiter,
        })
    }
}

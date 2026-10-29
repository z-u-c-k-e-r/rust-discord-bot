use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct Metrics {
    pub process_start_time: DateTime<Utc>,
    pub discord_events_total: AtomicU64,
    pub lua_commands_total: AtomicU64,
    pub lua_command_errors_total: AtomicU64,
    pub scheduler_jobs_claimed_total: AtomicU64,
    pub scheduler_jobs_delivered_total: AtomicU64,
    pub scheduler_jobs_failed_total: AtomicU64,
    pub dashboard_requests_total: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            process_start_time: Utc::now(),
            discord_events_total: AtomicU64::new(0),
            lua_commands_total: AtomicU64::new(0),
            lua_command_errors_total: AtomicU64::new(0),
            scheduler_jobs_claimed_total: AtomicU64::new(0),
            scheduler_jobs_delivered_total: AtomicU64::new(0),
            scheduler_jobs_failed_total: AtomicU64::new(0),
            dashboard_requests_total: AtomicU64::new(0),
        }
    }
}

impl Metrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn uptime_seconds(&self) -> i64 {
        (Utc::now() - self.process_start_time).num_seconds().max(0)
    }

    pub fn render_prometheus(&self, active_sessions: usize) -> String {
        format!(
            "# HELP zuckerbot_uptime_seconds Process uptime in seconds\n\
             # TYPE zuckerbot_uptime_seconds gauge\n\
             zuckerbot_uptime_seconds {}\n\
             # HELP zuckerbot_discord_events_total Total number of received Discord gateway events\n\
             # TYPE zuckerbot_discord_events_total counter\n\
             zuckerbot_discord_events_total {}\n\
             # HELP zuckerbot_lua_commands_total Total number of Lua commands executed\n\
             # TYPE zuckerbot_lua_commands_total counter\n\
             zuckerbot_lua_commands_total {}\n\
             # HELP zuckerbot_lua_command_errors_total Total number of failed Lua commands\n\
             # TYPE zuckerbot_lua_command_errors_total counter\n\
             zuckerbot_lua_command_errors_total {}\n\
             # HELP zuckerbot_scheduler_claimed_total Total number of scheduler jobs claimed\n\
             # TYPE zuckerbot_scheduler_claimed_total counter\n\
             zuckerbot_scheduler_claimed_total {}\n\
             # HELP zuckerbot_scheduler_delivered_total Total number of successfully delivered scheduled jobs\n\
             # TYPE zuckerbot_scheduler_delivered_total counter\n\
             zuckerbot_scheduler_delivered_total {}\n\
             # HELP zuckerbot_scheduler_failed_total Total number of failed scheduled jobs\n\
             # TYPE zuckerbot_scheduler_failed_total counter\n\
             zuckerbot_scheduler_failed_total {}\n\
             # HELP zuckerbot_dashboard_requests_total Total number of HTTP requests processed by dashboard\n\
             # TYPE zuckerbot_dashboard_requests_total counter\n\
             zuckerbot_dashboard_requests_total {}\n\
             # HELP zuckerbot_active_sessions Current number of active dashboard sessions\n\
             # TYPE zuckerbot_active_sessions gauge\n\
             zuckerbot_active_sessions {}\n",
            self.uptime_seconds(),
            self.discord_events_total.load(Ordering::Relaxed),
            self.lua_commands_total.load(Ordering::Relaxed),
            self.lua_command_errors_total.load(Ordering::Relaxed),
            self.scheduler_jobs_claimed_total.load(Ordering::Relaxed),
            self.scheduler_jobs_delivered_total.load(Ordering::Relaxed),
            self.scheduler_jobs_failed_total.load(Ordering::Relaxed),
            self.dashboard_requests_total.load(Ordering::Relaxed),
            active_sessions,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_prometheus_rendering() {
        let metrics = Metrics::new();
        metrics.discord_events_total.store(15, Ordering::Relaxed);
        metrics.lua_commands_total.store(7, Ordering::Relaxed);

        let rendered = metrics.render_prometheus(3);
        assert!(rendered.contains("zuckerbot_discord_events_total 15"));
        assert!(rendered.contains("zuckerbot_lua_commands_total 7"));
        assert!(rendered.contains("zuckerbot_active_sessions 3"));
        assert!(rendered.contains("zuckerbot_uptime_seconds"));
    }
}

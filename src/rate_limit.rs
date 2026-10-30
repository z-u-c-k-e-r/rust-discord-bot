use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RateLimitQuota {
    pub max_requests: u32,
    pub window_seconds: u64,
}

impl RateLimitQuota {
    pub const fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub reset_after: Duration,
    pub limit: u32,
}

#[derive(Clone, Debug)]
pub struct RateLimiter {
    policies: Arc<DashMap<String, RateLimitQuota>>,
    history: Arc<DashMap<String, Vec<Instant>>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        let policies = Arc::new(DashMap::new());

        // Default abuse-prevention policies:
        // Ticket creation: 3 per 5 minutes per user
        policies.insert("ticket_create".to_string(), RateLimitQuota::new(3, 5 * 60));
        // Music play / search: 10 per minute per user
        policies.insert("music_play".to_string(), RateLimitQuota::new(10, 60));
        policies.insert("music_search".to_string(), RateLimitQuota::new(10, 60));
        // Scheduler jobs: 20 per hour per user
        policies.insert(
            "scheduler_create".to_string(),
            RateLimitQuota::new(20, 60 * 60),
        );
        // Destructive / high-impact admin: 5 per minute per user
        policies.insert("destructive_admin".to_string(), RateLimitQuota::new(5, 60));
        // General command abuse prevention
        policies.insert("command_user".to_string(), RateLimitQuota::new(30, 60));
        policies.insert("command_guild".to_string(), RateLimitQuota::new(120, 60));
        // Web Admin API: 10 module updates per minute
        policies.insert(
            "admin:update_module".to_string(),
            RateLimitQuota::new(10, 60),
        );

        Self {
            policies,
            history: Arc::new(DashMap::new()),
        }
    }
}

impl RateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_quota(&self, scope: &str, quota: RateLimitQuota) {
        self.policies.insert(scope.to_string(), quota);
    }

    #[must_use]
    pub fn quota_for(&self, scope: &str) -> RateLimitQuota {
        self.policies
            .get(scope)
            .map(|q| *q.value())
            .unwrap_or(RateLimitQuota::new(30, 60))
    }

    fn evaluate_capacity(
        timestamps: &mut Vec<Instant>,
        now: Instant,
        quota: RateLimitQuota,
    ) -> Result<(), RateLimitDecision> {
        let window = Duration::from_secs(quota.window_seconds);
        timestamps.retain(|&ts| now.saturating_duration_since(ts) < window);

        if timestamps.len() >= quota.max_requests as usize {
            let earliest = timestamps.first().copied().unwrap_or(now);
            let elapsed = now.saturating_duration_since(earliest);
            let reset_after = window.saturating_sub(elapsed);
            Err(RateLimitDecision {
                allowed: false,
                remaining: 0,
                reset_after: if reset_after.is_zero() {
                    Duration::from_secs(1)
                } else {
                    reset_after
                },
                limit: quota.max_requests,
            })
        } else {
            Ok(())
        }
    }

    pub fn check_rate_limit(&self, key: &str, quota: RateLimitQuota) -> RateLimitDecision {
        let now = Instant::now();
        let mut entry = self.history.entry(key.to_string()).or_default();
        let timestamps = entry.value_mut();

        match Self::evaluate_capacity(timestamps, now, quota) {
            Ok(()) => {
                timestamps.push(now);
                let remaining = quota.max_requests.saturating_sub(timestamps.len() as u32);
                RateLimitDecision {
                    allowed: true,
                    remaining,
                    reset_after: Duration::ZERO,
                    limit: quota.max_requests,
                }
            }
            Err(decision) => decision,
        }
    }

    pub fn check_command(
        &self,
        command_name: &str,
        user_id: &str,
        guild_id: Option<&str>,
    ) -> RateLimitDecision {
        let cmd_scope = format!("cmd:{command_name}");
        let cmd_quota = self
            .policies
            .get(&cmd_scope)
            .map(|q| *q.value())
            .unwrap_or_else(|| self.quota_for("command_user"));
        let user_key = format!("user:{user_id}:cmd:{command_name}");

        let now = Instant::now();

        if let Some(gid) = guild_id {
            let guild_quota = self.quota_for("command_guild");
            let guild_key = format!("guild:{gid}:cmd");

            // Scope user capacity evaluation separately to prevent DashMap shard deadlock
            let user_decision = {
                let mut user_entry = self.history.entry(user_key.clone()).or_default();
                Self::evaluate_capacity(user_entry.value_mut(), now, cmd_quota)
            };
            if let Err(decision) = user_decision {
                return decision;
            }

            // Scope guild capacity evaluation separately
            let guild_decision = {
                let mut guild_entry = self.history.entry(guild_key.clone()).or_default();
                Self::evaluate_capacity(guild_entry.value_mut(), now, guild_quota)
            };
            if let Err(decision) = guild_decision {
                return decision;
            }

            // Record user timestamp
            let remaining = {
                let mut user_entry = self.history.entry(user_key).or_default();
                let user_timestamps = user_entry.value_mut();
                user_timestamps.push(now);
                cmd_quota
                    .max_requests
                    .saturating_sub(user_timestamps.len() as u32)
            };

            // Record guild timestamp
            {
                let mut guild_entry = self.history.entry(guild_key).or_default();
                guild_entry.value_mut().push(now);
            }

            RateLimitDecision {
                allowed: true,
                remaining,
                reset_after: Duration::ZERO,
                limit: cmd_quota.max_requests,
            }
        } else {
            self.check_rate_limit(&user_key, cmd_quota)
        }
    }

    pub fn check_action(
        &self,
        action_name: &str,
        user_id: &str,
        guild_id: Option<&str>,
    ) -> RateLimitDecision {
        let quota = self.quota_for(action_name);
        let key = if let Some(gid) = guild_id {
            format!("guild:{gid}:user:{user_id}:action:{action_name}")
        } else {
            format!("user:{user_id}:action:{action_name}")
        };
        self.check_rate_limit(&key, quota)
    }

    pub fn cleanup_stale_entries(&self) {
        let now = Instant::now();
        self.history.retain(|_, timestamps| {
            timestamps.retain(|&ts| now.saturating_duration_since(ts) < Duration::from_secs(3600));
            !timestamps.is_empty()
        });
    }
}

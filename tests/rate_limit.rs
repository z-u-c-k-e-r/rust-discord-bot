use std::time::Duration;
use zuckerbot::rate_limit::{RateLimitQuota, RateLimiter};

#[test]
fn test_rate_limiter_burst_and_refill() {
    let limiter = RateLimiter::new();
    // 3 requests allowed per 1-second window
    let quota = RateLimitQuota::new(3, 1);
    let key = "test_user_key";

    // 1st request -> allowed, 2 remaining
    let d1 = limiter.check_rate_limit(key, quota);
    assert!(d1.allowed);
    assert_eq!(d1.remaining, 2);
    assert_eq!(d1.limit, 3);

    // 2nd request -> allowed, 1 remaining
    let d2 = limiter.check_rate_limit(key, quota);
    assert!(d2.allowed);
    assert_eq!(d2.remaining, 1);

    // 3rd request -> allowed, 0 remaining
    let d3 = limiter.check_rate_limit(key, quota);
    assert!(d3.allowed);
    assert_eq!(d3.remaining, 0);

    // 4th request -> blocked (rate limit exceeded), reset_after > 0
    let d4 = limiter.check_rate_limit(key, quota);
    assert!(!d4.allowed);
    assert_eq!(d4.remaining, 0);
    assert!(d4.reset_after > Duration::ZERO);

    // Sleep until window expires
    std::thread::sleep(Duration::from_millis(1100));

    // 5th request -> allowed again after window slides!
    let d5 = limiter.check_rate_limit(key, quota);
    assert!(d5.allowed);
    assert_eq!(d5.remaining, 2);
}

#[test]
fn test_default_policies_and_custom_override() {
    let limiter = RateLimiter::new();

    // Default policy for ticket_create is 3 per 300s
    let q_ticket = limiter.quota_for("ticket_create");
    assert_eq!(q_ticket.max_requests, 3);
    assert_eq!(q_ticket.window_seconds, 300);

    // Default policy for music_play is 10 per 60s
    let q_music = limiter.quota_for("music_play");
    assert_eq!(q_music.max_requests, 10);
    assert_eq!(q_music.window_seconds, 60);

    // Default policy for destructive admin is conservative: 5 per 60s
    let q_admin = limiter.quota_for("destructive_admin");
    assert_eq!(q_admin.max_requests, 5);
    assert_eq!(q_admin.window_seconds, 60);

    // Custom policy override
    limiter.set_quota("ticket_create", RateLimitQuota::new(1, 10));
    let q_ticket_new = limiter.quota_for("ticket_create");
    assert_eq!(q_ticket_new.max_requests, 1);
    assert_eq!(q_ticket_new.window_seconds, 10);

    let d1 = limiter.check_action("ticket_create", "user_1", Some("guild_1"));
    assert!(d1.allowed);
    let d2 = limiter.check_action("ticket_create", "user_1", Some("guild_1"));
    assert!(!d2.allowed);
}

#[test]
fn test_per_user_and_per_guild_command_limits() {
    let limiter = RateLimiter::new();
    limiter.set_quota("command_user", RateLimitQuota::new(2, 60));
    limiter.set_quota("command_guild", RateLimitQuota::new(3, 60));

    let user_a = "user_a";
    let user_b = "user_b";
    let guild = "guild_shared";

    // User A executes 2 commands -> reaches per-user limit
    assert!(limiter.check_command("ping", user_a, Some(guild)).allowed);
    assert!(limiter.check_command("ping", user_a, Some(guild)).allowed);
    assert!(!limiter.check_command("ping", user_a, Some(guild)).allowed);

    // User B in same guild still has quota (1 remaining for guild)
    assert!(limiter.check_command("ping", user_b, Some(guild)).allowed);

    // Now guild limit (3) is reached!
    assert!(!limiter.check_command("ping", user_b, Some(guild)).allowed);
}

#[test]
fn test_stale_history_cleanup() {
    let limiter = RateLimiter::new();
    limiter.check_rate_limit("temp_key", RateLimitQuota::new(5, 60));
    limiter.cleanup_stale_entries();
    // Doesn't panic and maintains state
}

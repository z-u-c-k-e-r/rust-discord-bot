use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct MemberProgress {
    pub guild_id: String,
    pub user_id: String,
    pub xp: i64,
    pub level: i32,
    pub coins: i64,
    pub reputation: i64,
    pub messages: i64,
    pub daily_streak: i32,
    pub last_xp_at: Option<DateTime<Utc>>,
    pub last_daily_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MemberProgress {
    pub fn empty(guild_id: &str, user_id: &str, now: DateTime<Utc>) -> Self {
        Self {
            guild_id: guild_id.to_owned(),
            user_id: user_id.to_owned(),
            xp: 0,
            level: 0,
            coins: 0,
            reputation: 0,
            messages: 0,
            daily_streak: 0,
            last_xp_at: None,
            last_daily_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug)]
pub struct XpAward {
    pub profile: MemberProgress,
    pub awarded: i64,
    pub previous_level: i32,
}

impl XpAward {
    pub fn leveled_up(&self) -> bool {
        self.profile.level > self.previous_level
    }
}

#[derive(Clone, Debug)]
pub enum DailyClaimOutcome {
    Claimed {
        profile: MemberProgress,
        reward: i64,
        next_claim_at: DateTime<Utc>,
    },
    Cooldown {
        profile: MemberProgress,
        next_claim_at: DateTime<Utc>,
    },
}

#[derive(Clone, Debug)]
pub struct CoinTransfer {
    pub sender: MemberProgress,
    pub recipient: MemberProgress,
    pub amount: i64,
}

#[derive(Clone, Debug)]
pub enum CoinTransferOutcome {
    Completed(CoinTransfer),
    InsufficientFunds { balance: i64 },
    SameAccount,
}

#[derive(Clone, Debug)]
pub enum ReputationGrantOutcome {
    Granted {
        profile: MemberProgress,
        amount: i64,
        next_available_at: DateTime<Utc>,
    },
    Cooldown {
        next_available_at: DateTime<Utc>,
    },
    SameAccount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressMetric {
    Xp,
    Coins,
    Reputation,
    Messages,
}

impl ProgressMetric {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Xp => "XP",
            Self::Coins => "monet",
            Self::Reputation => "reputacji",
            Self::Messages => "wiadomości",
        }
    }

    pub const fn value(self, profile: &MemberProgress) -> i64 {
        match self {
            Self::Xp => profile.xp,
            Self::Coins => profile.coins,
            Self::Reputation => profile.reputation,
            Self::Messages => profile.messages,
        }
    }
}

pub fn level_for_xp(xp: i64) -> i32 {
    if xp <= 0 {
        return 0;
    }

    let level = ((1.0 + 0.08 * xp as f64).sqrt() - 1.0) / 2.0;
    level.floor().clamp(0.0, i32::MAX as f64) as i32
}

pub fn total_xp_for_level(level: i32) -> i64 {
    let level = i64::from(level.max(0));
    50_i64
        .saturating_mul(level)
        .saturating_mul(level.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::{level_for_xp, total_xp_for_level};

    #[test]
    fn level_curve_uses_increasing_triangular_requirements() {
        assert_eq!(level_for_xp(0), 0);
        assert_eq!(level_for_xp(99), 0);
        assert_eq!(level_for_xp(100), 1);
        assert_eq!(level_for_xp(299), 1);
        assert_eq!(level_for_xp(300), 2);
        assert_eq!(total_xp_for_level(0), 0);
        assert_eq!(total_xp_for_level(1), 100);
        assert_eq!(total_xp_for_level(2), 300);
        assert_eq!(total_xp_for_level(10), 5_500);
    }
}

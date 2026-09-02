use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressionOperation {
    AwardMessageXp {
        amount: u32,
        cooldown_seconds: u32,
        #[serde(default)]
        announce_level_up: bool,
        #[serde(default)]
        level_up_message: Option<String>,
    },
    Profile {
        #[serde(default)]
        user_id: Option<String>,
    },
    Balance {
        #[serde(default)]
        user_id: Option<String>,
    },
    Leaderboard {
        metric: ProgressionMetric,
        #[serde(default = "default_leaderboard_limit")]
        limit: u8,
    },
    Daily {
        base_reward: u32,
        #[serde(default)]
        streak_bonus: u32,
        #[serde(default = "default_max_streak_bonus")]
        max_streak_bonus: u16,
    },
    Transfer {
        user_id: String,
        amount: u64,
    },
    GiveReputation {
        user_id: String,
        #[serde(default = "default_reputation_amount")]
        amount: u16,
        #[serde(default = "default_reputation_cooldown")]
        cooldown_seconds: u32,
    },
    Adjust {
        user_id: String,
        #[serde(default)]
        xp_delta: i64,
        #[serde(default)]
        coins_delta: i64,
        #[serde(default)]
        reputation_delta: i64,
        #[serde(default)]
        reason: Option<String>,
    },
}

impl ProgressionOperation {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::AwardMessageXp {
                amount,
                cooldown_seconds,
                level_up_message,
                ..
            } => {
                if !(1..=1_000).contains(amount) {
                    return Err("message XP must be between 1 and 1000".to_owned());
                }
                if !(1..=86_400).contains(cooldown_seconds) {
                    return Err(
                        "message XP cooldown must be between 1 and 86400 seconds".to_owned()
                    );
                }
                if level_up_message
                    .as_ref()
                    .is_some_and(|message| message.is_empty() || message.chars().count() > 500)
                {
                    return Err("level-up message must contain 1 to 500 characters".to_owned());
                }
                Ok(())
            }
            Self::Profile { user_id } | Self::Balance { user_id } => {
                if let Some(user_id) = user_id {
                    validate_snowflake(user_id)?;
                }
                Ok(())
            }
            Self::Leaderboard { limit, .. } => {
                if !(1..=25).contains(limit) {
                    return Err("leaderboard limit must be between 1 and 25".to_owned());
                }
                Ok(())
            }
            Self::Daily {
                base_reward,
                streak_bonus,
                max_streak_bonus,
            } => {
                if *base_reward == 0 || *base_reward > 1_000_000 {
                    return Err("daily base reward must be between 1 and 1000000".to_owned());
                }
                if *streak_bonus > 100_000 {
                    return Err("daily streak bonus cannot exceed 100000".to_owned());
                }
                if *max_streak_bonus > 365 {
                    return Err("daily maximum streak bonus cannot exceed 365".to_owned());
                }
                Ok(())
            }
            Self::Transfer { user_id, amount } => {
                validate_snowflake(user_id)?;
                if *amount == 0 || *amount > 1_000_000_000_000 {
                    return Err("coin transfer must be between 1 and 1000000000000".to_owned());
                }
                Ok(())
            }
            Self::GiveReputation {
                user_id,
                amount,
                cooldown_seconds,
            } => {
                validate_snowflake(user_id)?;
                if !(1..=100).contains(amount) {
                    return Err("reputation amount must be between 1 and 100".to_owned());
                }
                if !(60..=604_800).contains(cooldown_seconds) {
                    return Err(
                        "reputation cooldown must be between 60 and 604800 seconds".to_owned()
                    );
                }
                Ok(())
            }
            Self::Adjust {
                user_id,
                xp_delta,
                coins_delta,
                reputation_delta,
                reason,
            } => {
                validate_snowflake(user_id)?;
                if *xp_delta == 0 && *coins_delta == 0 && *reputation_delta == 0 {
                    return Err("progress adjustment must change at least one value".to_owned());
                }
                for (name, value) in [
                    ("xp_delta", *xp_delta),
                    ("coins_delta", *coins_delta),
                    ("reputation_delta", *reputation_delta),
                ] {
                    if value.unsigned_abs() > 1_000_000_000 {
                        return Err(format!(
                            "{name} cannot exceed 1000000000 in either direction"
                        ));
                    }
                }
                if reason
                    .as_ref()
                    .is_some_and(|value| value.chars().count() > 512)
                {
                    return Err(
                        "progress adjustment reason cannot exceed 512 characters".to_owned()
                    );
                }
                Ok(())
            }
        }
    }

    pub const fn requires_manage_guild(&self) -> bool {
        matches!(self, Self::Adjust { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressionMetric {
    Xp,
    Coins,
    Reputation,
    Messages,
}

fn validate_snowflake(value: &str) -> Result<(), String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{value:?} is not a Discord snowflake"))?;
    if parsed == 0 {
        return Err("Discord snowflakes must be greater than zero".to_owned());
    }
    Ok(())
}

const fn default_leaderboard_limit() -> u8 {
    10
}

const fn default_max_streak_bonus() -> u16 {
    30
}

const fn default_reputation_amount() -> u16 {
    1
}

const fn default_reputation_cooldown() -> u32 {
    86_400
}

#[cfg(test)]
mod tests {
    use super::ProgressionOperation;

    #[test]
    fn rejects_unbounded_economy_operations() {
        assert!(
            ProgressionOperation::Transfer {
                user_id: "100000000000000001".to_owned(),
                amount: 0,
            }
            .validate()
            .is_err()
        );
        assert!(
            ProgressionOperation::Adjust {
                user_id: "100000000000000001".to_owned(),
                xp_delta: i64::MAX,
                coins_delta: 0,
                reputation_delta: 0,
                reason: None,
            }
            .validate()
            .is_err()
        );
    }
}

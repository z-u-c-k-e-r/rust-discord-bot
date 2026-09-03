use chrono::{DateTime, Duration, Utc};
use thiserror::Error;

const MAX_REPEAT_SECONDS: u64 = 31_557_600;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ScheduleTimeError {
    #[error("time value is empty")]
    Empty,
    #[error("time value is not a valid duration, Unix timestamp or RFC 3339 timestamp")]
    Invalid,
    #[error("duration contains an unsupported unit; use s, m, h, d or w")]
    UnsupportedUnit,
    #[error("duration is too large")]
    Overflow,
    #[error("scheduled time must be at least {minimum_seconds} seconds in the future")]
    TooSoon { minimum_seconds: u32 },
    #[error("scheduled time cannot be more than {maximum_seconds} seconds in the future")]
    TooFar { maximum_seconds: u64 },
    #[error("repeat interval must be between 60 seconds and one year")]
    InvalidRepeat,
}

pub fn parse_schedule_time(
    value: &str,
    now: DateTime<Utc>,
    minimum_delay_seconds: u32,
    maximum_delay_seconds: u64,
) -> Result<DateTime<Utc>, ScheduleTimeError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ScheduleTimeError::Empty);
    }

    let without_prefix = value
        .strip_prefix("in ")
        .or_else(|| value.strip_prefix("za "))
        .unwrap_or(value)
        .trim();

    let target = match parse_duration_seconds(without_prefix) {
        Ok(seconds) => now
            .checked_add_signed(Duration::seconds(
                i64::try_from(seconds).map_err(|_| ScheduleTimeError::Overflow)?,
            ))
            .ok_or(ScheduleTimeError::Overflow)?,
        Err(ScheduleTimeError::UnsupportedUnit | ScheduleTimeError::Overflow) => {
            return parse_absolute(value, now, minimum_delay_seconds, maximum_delay_seconds);
        }
        Err(_) => parse_absolute_timestamp(value)?,
    };

    validate_horizon(
        target,
        now,
        minimum_delay_seconds,
        maximum_delay_seconds,
    )
}

pub fn parse_repeat_interval(value: &str) -> Result<i64, ScheduleTimeError> {
    let seconds = parse_duration_seconds(value)?;
    if !(60..=MAX_REPEAT_SECONDS).contains(&seconds) {
        return Err(ScheduleTimeError::InvalidRepeat);
    }
    i64::try_from(seconds).map_err(|_| ScheduleTimeError::Overflow)
}

pub fn parse_duration_seconds(value: &str) -> Result<u64, ScheduleTimeError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Err(ScheduleTimeError::Empty);
    }

    let bytes = value.as_bytes();
    let mut index = 0;
    let mut total = 0_u64;
    let mut segments = 0_u32;

    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }

        let number_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if number_start == index || index >= bytes.len() {
            return Err(ScheduleTimeError::Invalid);
        }

        let amount = value[number_start..index]
            .parse::<u64>()
            .map_err(|_| ScheduleTimeError::Overflow)?;
        let multiplier = match bytes[index] {
            b's' => 1,
            b'm' => 60,
            b'h' => 3_600,
            b'd' => 86_400,
            b'w' => 604_800,
            _ => return Err(ScheduleTimeError::UnsupportedUnit),
        };
        index += 1;
        segments = segments.saturating_add(1);
        total = total
            .checked_add(
                amount
                    .checked_mul(multiplier)
                    .ok_or(ScheduleTimeError::Overflow)?,
            )
            .ok_or(ScheduleTimeError::Overflow)?;
    }

    if segments == 0 || total == 0 {
        return Err(ScheduleTimeError::Invalid);
    }
    Ok(total)
}

fn parse_absolute(
    value: &str,
    now: DateTime<Utc>,
    minimum_delay_seconds: u32,
    maximum_delay_seconds: u64,
) -> Result<DateTime<Utc>, ScheduleTimeError> {
    let target = parse_absolute_timestamp(value)?;
    validate_horizon(
        target,
        now,
        minimum_delay_seconds,
        maximum_delay_seconds,
    )
}

fn parse_absolute_timestamp(value: &str) -> Result<DateTime<Utc>, ScheduleTimeError> {
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        let timestamp = value
            .parse::<i64>()
            .map_err(|_| ScheduleTimeError::Invalid)?;
        return DateTime::from_timestamp(timestamp, 0).ok_or(ScheduleTimeError::Invalid);
    }

    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ScheduleTimeError::Invalid)
}

fn validate_horizon(
    target: DateTime<Utc>,
    now: DateTime<Utc>,
    minimum_delay_seconds: u32,
    maximum_delay_seconds: u64,
) -> Result<DateTime<Utc>, ScheduleTimeError> {
    let delay = target.signed_duration_since(now).num_seconds();
    if delay < i64::from(minimum_delay_seconds) {
        return Err(ScheduleTimeError::TooSoon {
            minimum_seconds: minimum_delay_seconds,
        });
    }

    let maximum = i64::try_from(maximum_delay_seconds).map_err(|_| ScheduleTimeError::Overflow)?;
    if delay > maximum {
        return Err(ScheduleTimeError::TooFar {
            maximum_seconds: maximum_delay_seconds,
        });
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{parse_duration_seconds, parse_repeat_interval, parse_schedule_time};

    #[test]
    fn parses_compound_relative_duration() {
        assert_eq!(parse_duration_seconds("1d 2h 30m"), Ok(95_400));
    }

    #[test]
    fn parses_rfc3339_and_unix_timestamps() {
        let now = Utc
            .with_ymd_and_hms(2026, 9, 3, 10, 0, 0)
            .single()
            .expect("valid test time");
        let rfc = parse_schedule_time("2026-09-03T12:00:00Z", now, 10, 86_400)
            .expect("RFC 3339 should parse");
        let unix = parse_schedule_time(&rfc.timestamp().to_string(), now, 10, 86_400)
            .expect("Unix timestamp should parse");
        assert_eq!(rfc, unix);
    }

    #[test]
    fn enforces_schedule_horizon() {
        let now = Utc
            .with_ymd_and_hms(2026, 9, 3, 10, 0, 0)
            .single()
            .expect("valid test time");
        assert!(parse_schedule_time("5s", now, 10, 86_400).is_err());
        assert!(parse_schedule_time("2d", now, 10, 86_400).is_err());
    }

    #[test]
    fn repeat_interval_is_bounded() {
        assert_eq!(parse_repeat_interval("1h"), Ok(3_600));
        assert!(parse_repeat_interval("30s").is_err());
    }
}

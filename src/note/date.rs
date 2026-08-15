use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(String);

impl Timestamp {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Timestamp {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Timestamp {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let valid_shape = value.len() == 20
            && value.as_bytes().get(4) == Some(&b'-')
            && value.as_bytes().get(7) == Some(&b'-')
            && value.as_bytes().get(10) == Some(&b'T')
            && value.as_bytes().get(13) == Some(&b':')
            && value.as_bytes().get(16) == Some(&b':')
            && value.as_bytes().get(19) == Some(&b'Z')
            && value.chars().enumerate().all(|(index, ch)| {
                matches!(index, 4 | 7) && ch == '-'
                    || index == 10 && ch == 'T'
                    || matches!(index, 13 | 16) && ch == ':'
                    || index == 19 && ch == 'Z'
                    || ch.is_ascii_digit()
            });
        if !valid_shape {
            return Err(invalid_timestamp(value));
        }

        let year: u32 = value[0..4].parse().unwrap_or(0);
        let month: u32 = value[5..7].parse().unwrap_or(0);
        let day: u32 = value[8..10].parse().unwrap_or(0);
        let hour: u8 = value[11..13].parse().unwrap_or(24);
        let minute: u8 = value[14..16].parse().unwrap_or(60);
        let second: u8 = value[17..19].parse().unwrap_or(60);
        if day == 0 || day > days_in_month(year, month) || hour > 23 || minute > 59 || second > 59 {
            return Err(invalid_timestamp(value));
        }

        Ok(Self(value.to_string()))
    }
}

pub fn timestamp_now() -> Result<Timestamp> {
    timestamp_from_system_time(SystemTime::now())
}

fn invalid_timestamp(value: &str) -> NtError {
    NtError::InvalidValue {
        field: "timestamp",
        value: value.to_string(),
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400) => 29,
        2 => 28,
        _ => 0,
    }
}

fn timestamp_from_system_time(time: SystemTime) -> Result<Timestamp> {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NtError::ClockOutOfRange)?
        .as_secs();
    let seconds = i64::try_from(seconds).map_err(|_| NtError::ClockOutOfRange)?;
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let second_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = second_of_day / 3600;
    let minute = (second_of_day % 3600) / 60;
    let second = second_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
        .parse()
        .map_err(|_| NtError::ClockOutOfRange)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{Timestamp, timestamp_from_system_time};
    use crate::error::NtError;

    #[test]
    fn formats_unix_epoch_timestamp() {
        let timestamp = timestamp_from_system_time(UNIX_EPOCH + Duration::from_secs(0)).unwrap();
        assert_eq!(timestamp.as_str(), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn timestamps_have_one_second_resolution() {
        let first = timestamp_from_system_time(UNIX_EPOCH + Duration::from_millis(1_000)).unwrap();
        let last = timestamp_from_system_time(UNIX_EPOCH + Duration::from_millis(1_999)).unwrap();
        assert_eq!(first, last);
        assert_eq!(first.as_str(), "1970-01-01T00:00:01Z");
    }

    #[test]
    fn rejects_system_times_before_the_unix_epoch() {
        assert!(matches!(
            timestamp_from_system_time(UNIX_EPOCH - Duration::from_secs(1)),
            Err(NtError::ClockOutOfRange)
        ));
    }

    #[test]
    fn parses_only_canonical_utc_seconds() {
        let timestamp: Timestamp = "2026-05-28T14:30:12Z".parse().unwrap();
        assert_eq!(timestamp.as_str(), "2026-05-28T14:30:12Z");
        for value in [
            "2026-02-29T14:30:12Z",
            "2026-05-28T24:00:00Z",
            "2026-05-28T14:30:12+00:00",
        ] {
            assert!(value.parse::<Timestamp>().is_err());
        }
    }
}

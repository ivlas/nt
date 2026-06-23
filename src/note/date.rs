use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timestamp {
    pub id: String,
    pub iso: String,
    pub day: String,
}

pub fn timestamp_now() -> Timestamp {
    timestamp_from_system_time(SystemTime::now())
}

pub fn local_day_now() -> String {
    std::process::Command::new("date")
        .arg("+%F")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|day| day.trim().to_string())
        .filter(|day| validate_date(day).is_ok())
        .unwrap_or_else(|| timestamp_now().day)
}

pub fn validate_date(value: &str) -> Result<()> {
    let valid_shape = value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value
            .chars()
            .enumerate()
            .all(|(index, ch)| matches!(index, 4 | 7) || ch.is_ascii_digit());
    if !valid_shape {
        return Err(invalid_date(value));
    }

    let year = value[0..4].parse().unwrap_or(0);
    let month = value[5..7].parse().unwrap_or(0);
    let day = value[8..10].parse().unwrap_or(0);
    let max_day = days_in_month(year, month);
    if day == 0 || day > max_day {
        return Err(invalid_date(value));
    }
    Ok(())
}

fn invalid_date(value: &str) -> NtError {
    NtError::Message(format!("invalid date `{value}`; use YYYY-MM-DD"))
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

pub fn add_days(day: &str, count: i64) -> Result<String> {
    validate_date(day)?;
    let year: i64 = day[0..4].parse().unwrap_or(0);
    let month: i64 = day[5..7].parse().unwrap_or(0);
    let date: i64 = day[8..10].parse().unwrap_or(0);
    let days = days_from_civil(year, month, date) + count;
    let (year, month, date) = civil_from_days(days);
    Ok(format!("{year:04}-{month:02}-{date:02}"))
}

pub fn timestamp_from_system_time(time: SystemTime) -> Timestamp {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    timestamp_from_unix_seconds(seconds)
}

fn timestamp_from_unix_seconds(seconds: i64) -> Timestamp {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let second_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = second_of_day / 3600;
    let minute = (second_of_day % 3600) / 60;
    let second = second_of_day % 60;

    Timestamp {
        id: format!("NT{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}"),
        iso: format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
        day: format!("{year:04}-{month:02}-{day:02}"),
    }
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    (year, m, d)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * adjusted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{add_days, timestamp_from_system_time, validate_date};

    #[test]
    fn validates_calendar_dates_and_adds_days() {
        validate_date("2024-02-29").unwrap();
        assert!(validate_date("2026-02-29").is_err());
        assert_eq!(add_days("2026-12-29", 6).unwrap(), "2027-01-04");
    }

    #[test]
    fn formats_unix_epoch_timestamp() {
        let timestamp = timestamp_from_system_time(UNIX_EPOCH + Duration::from_secs(0));
        assert_eq!(timestamp.id, "NT19700101T000000");
        assert_eq!(timestamp.iso, "1970-01-01T00:00:00Z");
        assert_eq!(timestamp.day, "1970-01-01");
    }
}

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{NtError, Result};
use crate::index::Index;

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

pub fn iso_from_id(id: &str) -> Result<String> {
    validate_id(id)?;
    Ok(format!(
        "{}-{}-{}T{}:{}:{}Z",
        &id[2..6],
        &id[6..8],
        &id[8..10],
        &id[11..13],
        &id[13..15],
        &id[15..17]
    ))
}

pub fn generate_unique_id(notes_dir: &Path, index: &Index) -> Result<Timestamp> {
    for _ in 0..5 {
        let timestamp = timestamp_now();
        if !notes_dir.join(format!("{}.md", timestamp.id)).exists()
            && !index.notes.contains_key(&timestamp.id)
        {
            return Ok(timestamp);
        }

        thread::sleep(Duration::from_secs(1));
    }

    Err(NtError::Message(
        "could not allocate a unique note id".to_string(),
    ))
}

pub fn note_path(notes_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_id(id)?;
    Ok(notes_dir.join(format!("{id}.md")))
}

pub fn validate_id(id: &str) -> Result<()> {
    if id.len() != 17 || !id.starts_with("NT") {
        return Err(NtError::InvalidNoteId(id.to_string()));
    }

    let bytes = id.as_bytes();
    if bytes[10] != b'T' {
        return Err(NtError::InvalidNoteId(id.to_string()));
    }

    if id[2..10].chars().all(|char| char.is_ascii_digit())
        && id[11..17].chars().all(|char| char.is_ascii_digit())
    {
        Ok(())
    } else {
        Err(NtError::InvalidNoteId(id.to_string()))
    }
}

pub fn title_from_body(body: &str) -> Result<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let title = trimmed
            .strip_prefix("# ")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .ok_or(NtError::InvalidTitle)?;
        return Ok(title.to_string());
    }

    Err(NtError::InvalidTitle)
}

pub fn sources_from_body(body: &str) -> Vec<String> {
    let mut sources = Vec::new();
    let mut cursor = 0;

    while cursor < body.len() {
        let Some(offset) = next_url_offset(&body[cursor..]) else {
            break;
        };
        let start = cursor + offset;
        let end = body[start..]
            .char_indices()
            .find_map(|(index, ch)| url_terminator(ch).then_some(start + index))
            .unwrap_or(body.len());
        let source = body[start..end].trim_end_matches(trailing_url_punctuation);
        if !source.is_empty() && !sources.iter().any(|value| value == source) {
            sources.push(source.to_string());
        }
        cursor = end.max(start + 1);
    }

    sources.sort();
    sources
}

fn next_url_offset(text: &str) -> Option<usize> {
    match (text.find("http://"), text.find("https://")) {
        (Some(http), Some(https)) => Some(http.min(https)),
        (Some(http), None) => Some(http),
        (None, Some(https)) => Some(https),
        (None, None) => None,
    }
}

fn url_terminator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ')' | ']' | '>' | '"' | '\'')
}

fn trailing_url_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | ':' | ';' | '!' | '?')
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

    use super::{
        add_days, generate_unique_id, sources_from_body, timestamp_from_system_time, timestamp_now,
        title_from_body, validate_date, validate_id,
    };

    #[test]
    fn validates_note_id_shape() {
        validate_id("NT20260528T143012").unwrap();
        assert!(validate_id("20260528T143012").is_err());
        assert!(validate_id("NT20260528-143012").is_err());
        assert!(validate_id("nt20260528T143012").is_err());
        assert!(validate_id("NT20260528T14301").is_err());
        assert!(validate_id("NT20260528T1430123").is_err());
        assert!(validate_id("NT2026AB28T143012").is_err());
        assert!(validate_id("NT20260528T14AB12").is_err());
        assert!(validate_id("NT20260528 143012").is_err());
    }

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

    #[test]
    fn extracts_title_from_markdown_heading() {
        assert_eq!(title_from_body("\n# Hello\nbody").unwrap(), "Hello");
    }

    #[test]
    fn requires_h1_title_as_first_non_empty_line() {
        assert!(title_from_body("body\n# Later").is_err());
        assert!(title_from_body("## Section\nbody").is_err());
        assert!(title_from_body("#\nbody").is_err());
        assert!(title_from_body("#   \nbody").is_err());
    }

    #[test]
    fn extracts_http_sources_from_markdown_body() {
        let body = "# Links\n\n[Spec](https://example.com/spec), <http://example.com/a>.\n";

        assert_eq!(
            sources_from_body(body),
            vec![
                "http://example.com/a".to_string(),
                "https://example.com/spec".to_string()
            ]
        );
    }

    #[test]
    fn generate_unique_id_retries_past_file_collision() {
        use crate::index::Index;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("nt-test-id-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let now = timestamp_now();
        fs::write(dir.join(format!("{}.md", now.id)), "# Collision\n").unwrap();

        let generated = generate_unique_id(&dir, &Index::default()).unwrap();
        assert_ne!(generated.id, now.id, "should retry past file collision");
        assert!(validate_id(&generated.id).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_unique_id_retries_past_index_collision() {
        use crate::index::{Index, NoteMeta};
        use std::fs;
        use std::path::PathBuf;

        let dir =
            std::env::temp_dir().join(format!("nt-test-index-collision-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let now = timestamp_now();
        let mut index = Index::default();
        index.notes.insert(
            now.id.clone(),
            NoteMeta::new_note(
                now.id.clone(),
                PathBuf::from(format!("notes/{}.md", now.id)),
                now.iso.clone(),
                now.iso.clone(),
                "Collision".to_string(),
            ),
        );

        let generated = generate_unique_id(&dir, &index).unwrap();
        assert_ne!(generated.id, now.id, "should retry past index collision");
        assert!(validate_id(&generated.id).is_ok());

        let _ = fs::remove_dir_all(&dir);
    }
}

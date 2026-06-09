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

pub fn title_from_body(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let title = trimmed.trim_start_matches('#').trim();
        if !title.is_empty() {
            return title.chars().take(80).collect();
        }
    }

    "(untitled)".to_string()
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{sources_from_body, timestamp_from_system_time, title_from_body, validate_id};

    #[test]
    fn validates_note_id_shape() {
        validate_id("NT20260528T143012").unwrap();
        assert!(validate_id("20260528T143012").is_err());
        assert!(validate_id("NT20260528-143012").is_err());
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
        assert_eq!(title_from_body("\n# Hello\nbody"), "Hello");
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
}

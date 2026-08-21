use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::{ContextV7, Timestamp as UuidTimestamp, Uuid, Version};

use crate::error::{NtError, Result};

const UUID_V7_MAX_MILLIS: u128 = (1_u128 << 48) - 1;
const SECONDS_PER_DAY: i64 = 86_400;
static UUID_V7_CONTEXT: Mutex<ContextV7> = Mutex::new(ContextV7::new());

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LibraryItemId(Uuid);

impl LibraryItemId {
    pub fn generate() -> Result<Self> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| NtError::ClockOutOfRange)?;
        let millis = u128::from(elapsed.as_secs()) * 1_000 + u128::from(elapsed.subsec_millis());
        if millis > UUID_V7_MAX_MILLIS {
            return Err(NtError::ClockOutOfRange);
        }
        let timestamp =
            UuidTimestamp::from_unix(&UUID_V7_CONTEXT, elapsed.as_secs(), elapsed.subsec_nanos());
        Ok(Self(Uuid::new_v7(timestamp)))
    }
}

impl fmt::Display for LibraryItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for LibraryItemId {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let uuid =
            Uuid::parse_str(value).map_err(|_| NtError::InvalidLibraryItemId(value.to_string()))?;
        if uuid.get_version() != Some(Version::SortRand) || uuid.to_string() != value {
            return Err(NtError::InvalidLibraryItemId(value.to_string()));
        }
        Ok(Self(uuid))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LibrarySource(String);

impl LibrarySource {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        ensure_nonempty("library source", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LibraryTimestamp(String);

impl LibraryTimestamp {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryTimestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for LibraryTimestamp {
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
            return Err(invalid_value("library timestamp", value));
        }
        let year: u32 = value[0..4].parse().unwrap_or(0);
        let month: u32 = value[5..7].parse().unwrap_or(0);
        let day: u32 = value[8..10].parse().unwrap_or(0);
        let hour: u8 = value[11..13].parse().unwrap_or(24);
        let minute: u8 = value[14..16].parse().unwrap_or(60);
        let second: u8 = value[17..19].parse().unwrap_or(60);
        if day == 0 || day > days_in_month(year, month) || hour > 23 || minute > 59 || second > 59 {
            return Err(invalid_value("library timestamp", value));
        }
        Ok(Self(value.to_string()))
    }
}

pub(crate) fn timestamp_now() -> Result<LibraryTimestamp> {
    let seconds = SystemTime::now()
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewLibraryCapture {
    content: String,
}

impl NewLibraryCapture {
    pub fn new(content: impl Into<String>) -> Result<Self> {
        let content = content.into();
        ensure_nonempty("library content", &content)?;
        Ok(Self { content })
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewLibraryItem {
    source: LibrarySource,
    title: String,
    capture: NewLibraryCapture,
}

impl NewLibraryItem {
    pub fn new(
        source: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self> {
        let title = title.into();
        ensure_nonempty("library title", &title)?;
        Ok(Self {
            source: LibrarySource::new(source)?,
            title,
            capture: NewLibraryCapture::new(content)?,
        })
    }

    pub fn source(&self) -> &LibrarySource {
        &self.source
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn capture(&self) -> &NewLibraryCapture {
        &self.capture
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryItem {
    id: LibraryItemId,
    source: LibrarySource,
    title: String,
    created: LibraryTimestamp,
    updated: LibraryTimestamp,
}

impl LibraryItem {
    pub(crate) fn rehydrate(
        id: LibraryItemId,
        source: String,
        title: String,
        created: LibraryTimestamp,
        updated: LibraryTimestamp,
    ) -> Result<Self> {
        ensure_nonempty("library title", &title)?;
        Ok(Self {
            id,
            source: LibrarySource::new(source)?,
            title,
            created,
            updated,
        })
    }

    pub fn id(&self) -> &LibraryItemId {
        &self.id
    }
    pub fn source(&self) -> &LibrarySource {
        &self.source
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn created(&self) -> &LibraryTimestamp {
        &self.created
    }
    pub fn updated(&self) -> &LibraryTimestamp {
        &self.updated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryCapture {
    pub(crate) pk: i64,
    captured: LibraryTimestamp,
    content: String,
    content_hash: String,
}

impl LibraryCapture {
    pub(crate) fn rehydrate(
        pk: i64,
        captured: LibraryTimestamp,
        content: String,
        content_hash: String,
    ) -> Result<Self> {
        ensure_nonempty("library content", &content)?;
        if content_hash.len() != 64
            || content_hash
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(invalid_value("library content hash", &content_hash));
        }
        Ok(Self {
            pk,
            captured,
            content,
            content_hash,
        })
    }

    pub fn captured(&self) -> &LibraryTimestamp {
        &self.captured
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibrarySummary {
    summary: String,
    generator: String,
    version: String,
    created: LibraryTimestamp,
}

impl LibrarySummary {
    pub(crate) fn rehydrate(
        summary: String,
        generator: String,
        version: String,
        created: LibraryTimestamp,
    ) -> Result<Self> {
        ensure_nonempty("library summary", &summary)?;
        ensure_nonempty("summary generator", &generator)?;
        ensure_nonempty("summary version", &version)?;
        Ok(Self {
            summary,
            generator,
            version,
            created,
        })
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }
    pub fn generator(&self) -> &str {
        &self.generator
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn created(&self) -> &LibraryTimestamp {
        &self.created
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibrarySummaryRow {
    item: LibraryItem,
    captured: LibraryTimestamp,
    summary: Option<String>,
}

impl LibrarySummaryRow {
    pub(crate) fn new(
        item: LibraryItem,
        captured: LibraryTimestamp,
        summary: Option<String>,
    ) -> Self {
        Self {
            item,
            captured,
            summary,
        }
    }
    pub fn item(&self) -> &LibraryItem {
        &self.item
    }
    pub fn captured(&self) -> &LibraryTimestamp {
        &self.captured
    }
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryHistoryRow {
    capture: LibraryCapture,
    summary: Option<LibrarySummary>,
}

impl LibraryHistoryRow {
    pub(crate) fn new(capture: LibraryCapture, summary: Option<LibrarySummary>) -> Self {
        Self { capture, summary }
    }
    pub fn capture(&self) -> &LibraryCapture {
        &self.capture
    }
    pub fn summary(&self) -> Option<&LibrarySummary> {
        self.summary.as_ref()
    }
}

fn ensure_nonempty(field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(invalid_value(field, value))
    } else {
        Ok(())
    }
}

fn invalid_value(field: &'static str, value: &str) -> NtError {
    NtError::InvalidValue {
        field,
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
    use super::*;

    #[test]
    fn validates_item_values_and_uuid_identity() {
        assert!(NewLibraryItem::new("https://example.com", "Example", "content").is_ok());
        assert!(NewLibraryItem::new("", "Example", "content").is_err());
        assert!(NewLibraryItem::new("source", " ", "content").is_err());
        assert!(NewLibraryItem::new("source", "Example", "").is_err());

        let id = LibraryItemId::generate().unwrap();
        assert_eq!(id.to_string().parse::<LibraryItemId>().unwrap(), id);
        assert!(
            "550e8400-e29b-41d4-a716-446655440000"
                .parse::<LibraryItemId>()
                .is_err()
        );
        assert!(
            "018FBE0A-6C00-7000-8000-000000000001"
                .parse::<LibraryItemId>()
                .is_err()
        );
    }
}

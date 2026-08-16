use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::{ContextV7, Timestamp as UuidTimestamp, Uuid, Version};

use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NoteId(Uuid);

const UUID_V7_MAX_MILLIS: u128 = (1_u128 << 48) - 1;
static UUID_V7_CONTEXT: Mutex<ContextV7> = Mutex::new(ContextV7::new());

impl NoteId {
    pub fn generate() -> Result<Self> {
        Self::generate_at(SystemTime::now(), &UUID_V7_CONTEXT)
    }

    fn generate_at(time: SystemTime, context: &Mutex<ContextV7>) -> Result<Self> {
        let elapsed = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| NtError::ClockOutOfRange)?;
        ensure_uuid_v7_range(elapsed.as_secs(), elapsed.subsec_nanos())?;

        let timestamp =
            UuidTimestamp::from_unix(context, elapsed.as_secs(), elapsed.subsec_nanos());
        let (seconds, subsec_nanos) = timestamp.to_unix();
        ensure_uuid_v7_range(seconds, subsec_nanos)?;

        Ok(Self(Uuid::new_v7(timestamp)))
    }
}

fn ensure_uuid_v7_range(seconds: u64, subsec_nanos: u32) -> Result<()> {
    let millis = u128::from(seconds) * 1_000 + u128::from(subsec_nanos / 1_000_000);
    if millis > UUID_V7_MAX_MILLIS {
        return Err(NtError::ClockOutOfRange);
    }
    Ok(())
}

impl fmt::Display for NoteId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let uuid = Uuid::parse_str(value).map_err(|_| NtError::InvalidNoteId(value.to_string()))?;
        if uuid.get_version() != Some(Version::SortRand) || uuid.to_string() != value {
            return Err(NtError::InvalidNoteId(value.to_string()));
        }
        Ok(Self(uuid))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, UNIX_EPOCH};

    use uuid::ContextV7;

    use super::{NoteId, UUID_V7_MAX_MILLIS};
    use crate::error::NtError;

    #[test]
    fn generates_canonical_uuid_v7_ids() {
        let id = NoteId::generate().unwrap();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text.as_bytes()[14], b'7');
        assert_eq!(text.parse::<NoteId>().unwrap(), id);
    }

    #[test]
    fn generation_rejects_times_outside_the_uuid_v7_range() {
        let context = Mutex::new(ContextV7::new());
        assert!(matches!(
            NoteId::generate_at(UNIX_EPOCH - Duration::from_secs(1), &context),
            Err(NtError::ClockOutOfRange)
        ));

        let after_maximum = UNIX_EPOCH + Duration::from_millis(UUID_V7_MAX_MILLIS as u64 + 1);
        assert!(matches!(
            NoteId::generate_at(after_maximum, &context),
            Err(NtError::ClockOutOfRange)
        ));
    }

    #[test]
    fn generation_preserves_order_within_one_millisecond() {
        let context = Mutex::new(ContextV7::new());
        let time = UNIX_EPOCH + Duration::from_secs(1_000);
        let first = NoteId::generate_at(time, &context).unwrap();
        let second = NoteId::generate_at(time, &context).unwrap();

        assert!(first < second);
    }

    #[test]
    fn rejects_noncanonical_and_non_v7_ids() {
        assert!("NT20260528T143012".parse::<NoteId>().is_err());
        assert!(
            "550e8400-e29b-41d4-a716-446655440000"
                .parse::<NoteId>()
                .is_err()
        );
        assert!(
            "018FBE0A-6C00-7000-8000-000000000001"
                .parse::<NoteId>()
                .is_err()
        );
    }
}

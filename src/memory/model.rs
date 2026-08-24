use crate::error::{NtError, Result};
use crate::note::Timestamp;

use super::{MEMORY_BODY_MAX_CHARS, MemoryRange};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewMemory(String);

impl NewMemory {
    pub(crate) fn new(body: impl AsRef<str>) -> Result<Self> {
        validate_input(body.as_ref(), "memory body").map(Self)
    }

    pub(crate) fn body(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewSummary(String);

impl NewSummary {
    pub(crate) fn new(body: impl AsRef<str>) -> Result<Self> {
        validate_input(body.as_ref(), "memory summary").map(Self)
    }

    pub(crate) fn body(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Memory {
    seq: u64,
    body: String,
}

impl Memory {
    pub(crate) fn from_stored(seq: i64, created: String, body: String) -> Result<Self> {
        let seq = u64::try_from(seq).map_err(|error| {
            NtError::invalid_stored_memory_with_source("identity: unknown", "sequence", error)
        })?;
        let identity = format!("#{seq}");
        let _created: Timestamp = created.parse().map_err(|error| {
            NtError::invalid_stored_memory_with_source(&identity, "created_at", error)
        })?;
        validate_stored(&body, "memory body").map_err(|error| {
            NtError::invalid_stored_memory_with_source(&identity, "body", error)
        })?;
        Ok(Self { seq, body })
    }

    pub(crate) fn seq(&self) -> u64 {
        self.seq
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Summary {
    range: MemoryRange,
    body: String,
}

impl Summary {
    pub(crate) fn from_stored(lo: i64, hi: i64, body: String) -> Result<Self> {
        let identity = format!("summary {lo}-{hi}");
        let lo = u64::try_from(lo)
            .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "lo", error))?;
        let hi = u64::try_from(hi)
            .map_err(|error| NtError::invalid_stored_memory_with_source(&identity, "hi", error))?;
        let range = MemoryRange::new(lo, hi).map_err(|error| {
            NtError::invalid_stored_memory_with_source(&identity, "range", error)
        })?;
        validate_stored(&body, "memory summary").map_err(|error| {
            NtError::invalid_stored_memory_with_source(&identity, "body", error)
        })?;
        Ok(Self { range, body })
    }

    pub(crate) fn range(&self) -> MemoryRange {
        self.range
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }
}

fn validate_input(body: &str, field: &'static str) -> Result<String> {
    let body = body
        .strip_suffix("\r\n")
        .or_else(|| body.strip_suffix('\n'))
        .unwrap_or(body);
    validate_stored(body, field)?;
    Ok(body.to_string())
}

fn validate_stored(body: &str, field: &'static str) -> Result<()> {
    if body.is_empty() {
        return Err(NtError::EmptyBody);
    }
    if body.contains(['\r', '\n']) {
        return Err(NtError::InvalidValue {
            field,
            value: "must be one line".to_string(),
        });
    }
    if body.contains('\0') {
        return Err(NtError::InvalidValue {
            field,
            value: "contains NUL".to_string(),
        });
    }
    if body.chars().take(MEMORY_BODY_MAX_CHARS + 1).count() > MEMORY_BODY_MAX_CHARS {
        return Err(NtError::InvalidValue {
            field,
            value: format!("exceeds {MEMORY_BODY_MAX_CHARS} characters"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NewMemory, NewSummary};
    use crate::error::NtError;

    #[test]
    fn memory_bodies_are_short_nonempty_single_lines() {
        assert_eq!(NewMemory::new("concise").unwrap().body(), "concise");
        assert!(NewMemory::new("e".repeat(512)).is_ok());
        assert!(NewSummary::new("é".repeat(512)).is_ok());

        for invalid in ["", "two\nlines", "carriage\rreturn", "nul\0byte"] {
            assert!(NewMemory::new(invalid).is_err(), "accepted {invalid:?}");
        }
        assert!(matches!(
            NewMemory::new("é".repeat(513)),
            Err(NtError::InvalidValue {
                field: "memory body",
                ..
            })
        ));
    }
}

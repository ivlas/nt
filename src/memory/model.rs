use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};
use crate::note::Timestamp;

use super::{MEMORY_ENTRY_MAX_CHARS, MEMORY_SUMMARY_MAX_CHARS};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewMemory {
    body: String,
}

impl NewMemory {
    pub(crate) fn new(body: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            body: normalize_and_validate(
                body.as_ref(),
                "memory body",
                MEMORY_ENTRY_MAX_CHARS,
                true,
            )?,
        })
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    fn into_body(self) -> String {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewSummary {
    summary: String,
}

impl NewSummary {
    pub(crate) fn new(summary: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            summary: normalize_and_validate(
                summary.as_ref(),
                "memory summary",
                MEMORY_SUMMARY_MAX_CHARS,
                false,
            )?,
        })
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    fn into_summary(self) -> String {
        self.summary
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SummaryNodeId {
    level: u64,
    block: u64,
}

impl SummaryNodeId {
    pub(crate) fn new(level: u64, block: u64) -> Result<Self> {
        if level > i64::MAX as u64 || block > i64::MAX as u64 {
            return Err(invalid_node(format!("L{level}:{block}")));
        }
        Ok(Self { level, block })
    }

    pub(crate) fn level(self) -> u64 {
        self.level
    }

    pub(crate) fn block(self) -> u64 {
        self.block
    }
}

impl fmt::Display for SummaryNodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "L{}:{}", self.level, self.block)
    }
}

impl FromStr for SummaryNodeId {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let Some((level, block)) = value
            .strip_prefix('L')
            .and_then(|rest| rest.split_once(':'))
        else {
            return Err(invalid_node(value.to_string()));
        };
        if !is_canonical_integer(level) || !is_canonical_integer(block) {
            return Err(invalid_node(value.to_string()));
        }
        let level = level
            .parse::<u64>()
            .map_err(|_| invalid_node(value.to_string()))?;
        let block = block
            .parse::<u64>()
            .map_err(|_| invalid_node(value.to_string()))?;
        Self::new(level, block).map_err(|_| invalid_node(value.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Memory {
    seq: i64,
    body: String,
    created: Timestamp,
}

impl Memory {
    pub(crate) fn from_new(seq: i64, memory: NewMemory, created: Timestamp) -> Result<Self> {
        validate_positive_identity("memory seq", seq)?;
        Ok(Self {
            seq,
            body: memory.into_body(),
            created,
        })
    }

    pub(crate) fn from_stored(seq: i64, body: String, created: Timestamp) -> Result<Self> {
        let memory = NewMemory::new(&body)?;
        if memory.body() != body {
            return Err(NtError::InvalidValue {
                field: "memory body",
                value: "not LF-normalized".to_string(),
            });
        }
        Self::from_new(seq, memory, created)
    }

    pub(crate) fn seq(&self) -> i64 {
        self.seq
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) fn created(&self) -> &Timestamp {
        &self.created
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MemorySegment {
    pk: i64,
    node: SummaryNodeId,
    summary: String,
    created: Timestamp,
}

impl MemorySegment {
    pub(crate) fn from_new(
        pk: i64,
        node: SummaryNodeId,
        summary: NewSummary,
        created: Timestamp,
    ) -> Result<Self> {
        validate_positive_identity("memory segment pk", pk)?;
        Ok(Self {
            pk,
            node,
            summary: summary.into_summary(),
            created,
        })
    }

    pub(crate) fn from_stored(
        pk: i64,
        node: SummaryNodeId,
        summary: String,
        created: Timestamp,
    ) -> Result<Self> {
        let new_summary = NewSummary::new(&summary)?;
        if new_summary.summary() != summary {
            return Err(NtError::InvalidValue {
                field: "memory summary",
                value: "not LF-normalized".to_string(),
            });
        }
        Self::from_new(pk, node, new_summary, created)
    }

    #[cfg(test)]
    pub(crate) fn pk(&self) -> i64 {
        self.pk
    }

    pub(crate) fn node(&self) -> SummaryNodeId {
        self.node
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn created(&self) -> &Timestamp {
        &self.created
    }
}

fn normalize_and_validate(
    value: &str,
    field: &'static str,
    max_chars: usize,
    empty_is_body: bool,
) -> Result<String> {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() {
        return if empty_is_body {
            Err(NtError::EmptyBody)
        } else {
            Err(NtError::InvalidValue {
                field,
                value: "empty".to_string(),
            })
        };
    }
    if normalized.contains('\0') {
        return Err(NtError::InvalidValue {
            field,
            value: "contains NUL".to_string(),
        });
    }
    if normalized.chars().take(max_chars + 1).count() > max_chars {
        return Err(NtError::InvalidValue {
            field,
            value: format!("exceeds {max_chars} characters"),
        });
    }
    Ok(normalized)
}

fn validate_positive_identity(field: &'static str, value: i64) -> Result<()> {
    if value <= 0 {
        return Err(NtError::InvalidValue {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

fn is_canonical_integer(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn invalid_node(value: String) -> NtError {
    NtError::InvalidValue {
        field: "memory node",
        value,
    }
}

#[cfg(test)]
mod tests {
    use super::{Memory, MemorySegment, NewMemory, NewSummary, SummaryNodeId};
    use crate::error::NtError;
    use crate::note::Timestamp;

    fn timestamp() -> Timestamp {
        "2026-08-22T12:34:56Z".parse().unwrap()
    }

    #[test]
    fn inputs_normalize_newlines_and_count_unicode_characters() {
        let memory = NewMemory::new("alpha\r\nbeta\rgamma").unwrap();
        assert_eq!(memory.body(), "alpha\nbeta\ngamma");

        let summary = NewSummary::new("é".repeat(1_024)).unwrap();
        assert_eq!(summary.summary().chars().count(), 1_024);
        assert!(NewMemory::new("é".repeat(1_025)).is_err());
        assert!(NewSummary::new("é".repeat(1_025)).is_err());
    }

    #[test]
    fn inputs_reject_empty_and_nul_without_echoing_content() {
        assert!(matches!(NewMemory::new(""), Err(NtError::EmptyBody)));
        assert!(matches!(
            NewSummary::new(""),
            Err(NtError::InvalidValue {
                field: "memory summary",
                value,
            }) if value == "empty"
        ));
        let error = NewMemory::new("secret\0tail").unwrap_err();
        assert!(matches!(
            &error,
            NtError::InvalidValue {
                field: "memory body",
                value,
            } if value == "contains NUL"
        ));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn node_ids_parse_and_display_canonically() {
        let node: SummaryNodeId = "L12:345".parse().unwrap();
        assert_eq!(node.level(), 12);
        assert_eq!(node.block(), 345);
        assert_eq!(node.to_string(), "L12:345");

        for invalid in [
            "L0",
            "L:0",
            "L0:",
            "L00:0",
            "L0:00",
            "L-1:0",
            "l0:0",
            "L0:0:0",
            "L9223372036854775808:0",
            "L0:9223372036854775808",
        ] {
            assert!(
                invalid.parse::<SummaryNodeId>().is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn stored_models_are_immutable_validated_values() {
        let memory = Memory::from_new(1, NewMemory::new("body").unwrap(), timestamp()).unwrap();
        assert_eq!(memory.seq(), 1);
        assert_eq!(memory.body(), "body");
        assert_eq!(memory.created().as_str(), "2026-08-22T12:34:56Z");
        assert!(Memory::from_new(0, NewMemory::new("body").unwrap(), timestamp()).is_err());
        assert!(Memory::from_stored(1, "a\r\nb".to_string(), timestamp()).is_err());

        let node = SummaryNodeId::new(0, 0).unwrap();
        let segment =
            MemorySegment::from_new(1, node, NewSummary::new("summary").unwrap(), timestamp())
                .unwrap();
        assert_eq!(segment.pk(), 1);
        assert_eq!(segment.node(), node);
        assert_eq!(segment.summary(), "summary");
        assert_eq!(segment.created().as_str(), "2026-08-22T12:34:56Z");
        assert!(MemorySegment::from_stored(1, node, "a\rb".to_string(), timestamp()).is_err());
    }
}

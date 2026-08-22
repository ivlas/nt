use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CollectionPath(String);

impl CollectionPath {
    pub fn inbox() -> Self {
        Self("inbox".to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CollectionPath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CollectionPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CollectionPath {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        if value.is_empty() || value.split('/').any(segment_is_invalid) {
            return Err(NtError::InvalidValue {
                field: "collection",
                value: value.to_string(),
            });
        }
        Ok(Self(value.to_string()))
    }
}

pub(super) fn segment_is_invalid(value: &str) -> bool {
    value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::CollectionPath;

    #[test]
    fn accepts_normalized_collection_paths() {
        for value in ["inbox", "work/nt", "research/sqlite/index_2"] {
            let collection: CollectionPath = value.parse().unwrap();
            assert_eq!(collection.as_str(), value);
        }
        assert_eq!(CollectionPath::inbox().as_str(), "inbox");
    }

    #[test]
    fn rejects_invalid_collection_paths() {
        for value in ["", "/inbox", "inbox/", "work//nt", "Work/nt", "work/a.b"] {
            assert!(
                value.parse::<CollectionPath>().is_err(),
                "accepted {value:?}"
            );
        }
    }
}

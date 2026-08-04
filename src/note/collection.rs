use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QualifiedCollection(String);

impl QualifiedCollection {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn vault(&self) -> &str {
        self.0
            .split_once('/')
            .expect("qualified collection is validated")
            .0
    }

    pub fn collection(&self) -> &str {
        self.0
            .split_once('/')
            .expect("qualified collection is validated")
            .1
    }
}

impl AsRef<str> for QualifiedCollection {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for QualifiedCollection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for QualifiedCollection {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        let Some((vault, collection)) = value.split_once('/') else {
            return Err(NtError::Message(format!(
                "invalid collection `{value}`; use <vault>/<collection>"
            )));
        };
        validate_namespace_part(vault, "vault")?;
        validate_namespace_part(collection, "collection")?;
        Ok(Self(value.to_string()))
    }
}

pub(crate) fn validate_namespace_part(value: &str, kind: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_uppercase() || ch == ',')
    {
        return Err(NtError::Message(format!(
            "invalid {kind} `{value}`; use lowercase names without spaces or commas"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::QualifiedCollection;

    #[test]
    fn collection_names_are_vault_qualified() {
        let collection: QualifiedCollection = "personal/rust".parse().unwrap();
        assert_eq!(collection.vault(), "personal");
        assert_eq!(collection.collection(), "rust");
        assert_eq!(collection.as_str(), "personal/rust");
        assert!("rust".parse::<QualifiedCollection>().is_err());
        assert!("Personal/rust".parse::<QualifiedCollection>().is_err());
    }
}

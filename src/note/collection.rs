use std::fmt;
use std::str::FromStr;

use crate::error::{CollectionErrorKind, NtError, Result};

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
            return Err(NtError::InvalidCollection {
                value: value.to_string(),
                component: None,
                kind: CollectionErrorKind::MissingQualifier,
            });
        };
        validate_collection_part(value, vault, CollectionErrorKind::InvalidVault)?;
        validate_collection_part(value, collection, CollectionErrorKind::InvalidName)?;
        Ok(Self(value.to_string()))
    }
}

fn validate_collection_part(value: &str, component: &str, kind: CollectionErrorKind) -> Result<()> {
    if namespace_part_is_invalid(component) {
        return Err(NtError::InvalidCollection {
            value: value.to_string(),
            component: Some(component.to_string()),
            kind,
        });
    }
    Ok(())
}

pub(crate) fn validate_namespace_part(value: &str, kind: &str) -> Result<()> {
    if namespace_part_is_invalid(value) {
        return Err(NtError::Message(format!(
            "invalid {kind} `{value}`; use lowercase names without slashes, spaces, or commas"
        )));
    }
    Ok(())
}

fn namespace_part_is_invalid(value: &str) -> bool {
    value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_uppercase() || matches!(ch, ',' | '/'))
}

#[cfg(test)]
mod tests {
    use crate::error::{CollectionErrorKind, NtError};

    use super::QualifiedCollection;

    #[test]
    fn collection_names_are_vault_qualified() {
        let collection: QualifiedCollection = "personal/rust".parse().unwrap();
        assert_eq!(collection.vault(), "personal");
        assert_eq!(collection.collection(), "rust");
        assert_eq!(collection.as_str(), "personal/rust");
        assert!(matches!(
            "rust".parse::<QualifiedCollection>().unwrap_err(),
            NtError::InvalidCollection {
                value,
                component: None,
                kind: CollectionErrorKind::MissingQualifier,
            } if value == "rust"
        ));
        assert!("Personal/rust".parse::<QualifiedCollection>().is_err());
        assert!(
            "personal/rust/notes"
                .parse::<QualifiedCollection>()
                .is_err()
        );
        assert!("personal//rust".parse::<QualifiedCollection>().is_err());
    }

    #[test]
    fn collection_errors_identify_the_invalid_component() {
        assert!(matches!(
            "Personal/inbox"
                .parse::<QualifiedCollection>()
                .unwrap_err(),
            NtError::InvalidCollection {
                value,
                component: Some(component),
                kind: CollectionErrorKind::InvalidVault,
            } if value == "Personal/inbox" && component == "Personal"
        ));
        assert!(matches!(
            "personal/Rust"
                .parse::<QualifiedCollection>()
                .unwrap_err(),
            NtError::InvalidCollection {
                component: Some(component),
                kind: CollectionErrorKind::InvalidName,
                ..
            } if component == "Rust"
        ));
    }
}

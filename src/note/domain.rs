use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use crate::error::{NtError, Result};

use super::body::normalize_body;
use super::collection::segment_is_invalid;
use super::{CollectionPath, NoteId, Timestamp};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tag(String);

impl Tag {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Tag {
    type Err = NtError;

    fn from_str(value: &str) -> Result<Self> {
        if segment_is_invalid(value) {
            return Err(NtError::InvalidValue {
                field: "tag",
                value: value.to_string(),
            });
        }
        Ok(Self(value.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewNote {
    collection: CollectionPath,
    body: String,
    title: String,
    tags: BTreeSet<Tag>,
    links: BTreeSet<NoteId>,
}

impl NewNote {
    pub fn new(collection: CollectionPath, body: impl AsRef<str>) -> Result<Self> {
        let (body, title) = normalize_body(body.as_ref())?;
        Ok(Self {
            collection,
            body,
            title,
            tags: BTreeSet::new(),
            links: BTreeSet::new(),
        })
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = Tag>) -> Self {
        self.tags.extend(tags);
        self
    }

    pub fn with_links(mut self, links: impl IntoIterator<Item = NoteId>) -> Self {
        self.links.extend(links);
        self
    }

    pub fn validate_links_for(&self, id: &NoteId) -> Result<()> {
        if self.links.contains(id) {
            return Err(NtError::SelfLink);
        }
        Ok(())
    }

    pub fn collection(&self) -> &CollectionPath {
        &self.collection
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }

    pub fn links(&self) -> &BTreeSet<NoteId> {
        &self.links
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    id: NoteId,
    collection: CollectionPath,
    body: String,
    title: String,
    created: Timestamp,
    updated: Timestamp,
    body_version: u64,
    tags: BTreeSet<Tag>,
    links: BTreeSet<NoteId>,
}

impl Note {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rehydrate(
        id: NoteId,
        collection: CollectionPath,
        body: String,
        title: String,
        created: Timestamp,
        updated: Timestamp,
        body_version: u64,
        tags: BTreeSet<Tag>,
        links: BTreeSet<NoteId>,
    ) -> Result<Self> {
        let (normalized_body, derived_title) = normalize_body(&body)?;
        if normalized_body != body || derived_title != title {
            return Err(NtError::Message(
                "invalid stored note body or title".to_string(),
            ));
        }
        if body_version == 0 {
            return Err(NtError::InvalidBodyVersion(body_version));
        }
        if links.contains(&id) {
            return Err(NtError::SelfLink);
        }
        Ok(Self {
            id,
            collection,
            body,
            title,
            created,
            updated,
            body_version,
            tags,
            links,
        })
    }

    pub fn replace_body(&mut self, body: impl AsRef<str>, updated: Timestamp) -> Result<bool> {
        let (body, title) = normalize_body(body.as_ref())?;
        if body == self.body {
            return Ok(false);
        }
        self.body_version = self
            .body_version
            .checked_add(1)
            .ok_or(NtError::InvalidBodyVersion(self.body_version))?;
        self.body = body;
        self.title = title;
        self.updated = updated;
        Ok(true)
    }

    pub fn id(&self) -> &NoteId {
        &self.id
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn updated(&self) -> &Timestamp {
        &self.updated
    }

    pub fn body_version(&self) -> u64 {
        self.body_version
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{NewNote, Note, Tag};
    use crate::error::NtError;
    use crate::note::{CollectionPath, NoteId, Timestamp};

    fn id(value: &str) -> NoteId {
        value.parse().unwrap()
    }

    fn timestamp(value: &str) -> Timestamp {
        value.parse().unwrap()
    }

    #[test]
    fn tags_are_normalized_domain_values() {
        assert_eq!("rust_2026".parse::<Tag>().unwrap().as_str(), "rust_2026");
        for value in ["", "Rust", "rust/sqlite", "rust.sqlite"] {
            assert!(value.parse::<Tag>().is_err(), "accepted {value:?}");
        }
    }

    #[test]
    fn new_notes_normalize_body_and_deduplicate_sets() {
        let target = id("018fbe0a-6c00-7000-8000-000000000002");
        let note = NewNote::new(CollectionPath::inbox(), "# Title\r\nBody")
            .unwrap()
            .with_tags(["rust".parse().unwrap(), "rust".parse().unwrap()])
            .with_links([target.clone(), target]);
        assert_eq!(note.body(), "# Title\nBody");
        assert_eq!(note.title(), "Title");
        assert_eq!(note.collection().as_str(), "inbox");
        assert_eq!(note.tags().len(), 1);
        assert_eq!(note.links().len(), 1);
    }

    #[test]
    fn notes_reject_invalid_rehydration_and_self_links() {
        let note_id = id("018fbe0a-6c00-7000-8000-000000000001");
        let links = BTreeSet::from([note_id.clone()]);
        let result = Note::rehydrate(
            note_id.clone(),
            CollectionPath::inbox(),
            "# Title".to_string(),
            "Title".to_string(),
            timestamp("2026-05-28T14:30:12Z"),
            timestamp("2026-05-28T14:30:12Z"),
            1,
            BTreeSet::new(),
            links,
        );
        assert!(matches!(result, Err(NtError::SelfLink)));

        let new_note = NewNote::new(CollectionPath::inbox(), "# Title")
            .unwrap()
            .with_links([note_id.clone()]);
        assert!(matches!(
            new_note.validate_links_for(&note_id),
            Err(NtError::SelfLink)
        ));
    }

    #[test]
    fn body_replacement_updates_only_for_a_real_change() {
        let mut note = Note::rehydrate(
            id("018fbe0a-6c00-7000-8000-000000000001"),
            CollectionPath::inbox(),
            "# Title".to_string(),
            "Title".to_string(),
            timestamp("2026-05-28T14:30:12Z"),
            timestamp("2026-05-28T14:30:12Z"),
            1,
            BTreeSet::new(),
            BTreeSet::new(),
        )
        .unwrap();
        assert!(
            !note
                .replace_body("# Title", timestamp("2026-05-28T15:00:00Z"))
                .unwrap()
        );
        assert_eq!(note.body_version(), 1);
        assert!(
            note.replace_body("# Changed\n", timestamp("2026-05-28T15:00:00Z"))
                .unwrap()
        );
        assert_eq!(note.body(), "# Changed\n");
        assert_eq!(note.title(), "Changed");
        assert_eq!(note.body_version(), 2);
        assert_eq!(note.updated().as_str(), "2026-05-28T15:00:00Z");
    }
}

use std::collections::BTreeSet;

use rusqlite::{TransactionBehavior, params};

use super::super::{CollectionPath, NewNote, NoteId, NoteQuery, Tag, timestamp_now};
use crate::error::NtError;

use super::store::load_note;
use super::{AddOrRemove, NoteSummary, Repository};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::schema;
    use crate::storage::{InitOutcome, OpenMode};

    fn initialize_at(path: &std::path::Path) -> Result<InitOutcome> {
        Repository::initialize_at(path)
    }

    fn open_at(path: &std::path::Path, mode: OpenMode) -> Result<Repository> {
        match mode {
            OpenMode::ReadOnly => Repository::open_read_only(path),
            OpenMode::ReadWrite => Repository::open_at(path),
        }
    }

    fn repository() -> Repository {
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        schema::initialize(&mut connection).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON")
            .unwrap();
        Repository { connection }
    }

    fn summaries(repository: &Repository, query: &NoteQuery) -> Vec<NoteSummary> {
        let mut summaries = Vec::new();
        repository
            .visit_note_summaries(query, |summary| {
                summaries.push(summary);
                Ok(())
            })
            .unwrap();
        summaries
    }

    fn assert_invalid_summary(repository: &Repository, expected_field: &'static str) {
        assert!(matches!(
            repository.visit_note_summaries(&NoteQuery::default(), |_| Ok(())),
            Err(NtError::InvalidStoredNote { field, .. }) if field == expected_field
        ));
    }

    fn corrupt_repository(mutation: &str) -> (Repository, NoteId) {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Corrupt").unwrap())
            .unwrap();
        repository
            .connection
            .execute_batch("PRAGMA ignore_check_constraints = ON")
            .unwrap();
        repository.connection.execute_batch(mutation).unwrap();
        repository
            .connection
            .execute_batch("PRAGMA ignore_check_constraints = OFF")
            .unwrap();
        (repository, id)
    }

    #[test]
    fn creates_loads_lists_and_deletes_notes() {
        let mut repository = repository();
        let id = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Storage\nBody")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        let note = repository.get_note(&id).unwrap();
        assert_eq!(note.body(), "# Storage\nBody");

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id(), &id);
        assert_eq!(notes[0].tags().len(), 1);
        repository.delete_notes(std::slice::from_ref(&id)).unwrap();
        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::NoteNotFound(_))
        ));
    }

    #[test]
    fn invalid_persisted_body_versions_are_stored_note_errors() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Invalid version").unwrap())
            .unwrap();
        let mut changed_note = repository.get_note(&id).unwrap();
        let expected_version = changed_note.body_version();
        changed_note
            .replace_body("# Changed", timestamp_now().unwrap())
            .unwrap();
        repository
            .connection
            .execute_batch("PRAGMA ignore_check_constraints = ON")
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET body_version = -1 WHERE id = ?1",
                [id.to_string()],
            )
            .unwrap();
        repository
            .connection
            .execute_batch("PRAGMA ignore_check_constraints = OFF")
            .unwrap();

        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::InvalidStoredNote {
                field: "body_version",
                ..
            })
        ));
        assert!(matches!(
            repository.verify_body_version(&id, 1),
            Err(NtError::InvalidStoredNote {
                field: "body_version",
                ..
            })
        ));
        assert!(matches!(
            repository.replace_body(&changed_note, expected_version),
            Err(NtError::InvalidStoredNote {
                field: "body_version",
                ..
            })
        ));
    }

    #[test]
    fn invalid_persisted_full_note_metadata_are_stored_note_errors() {
        for (mutation, field) in [
            ("UPDATE notes SET collection = 'Invalid'", "collection"),
            ("UPDATE notes SET created = 'invalid'", "created"),
            ("UPDATE notes SET updated = 'invalid'", "updated"),
        ] {
            let (repository, id) = corrupt_repository(mutation);
            assert!(matches!(
                repository.get_note(&id),
                Err(NtError::InvalidStoredNote { field: actual, .. }) if actual == field
            ));
        }

        let (repository, id) = corrupt_repository(
            "INSERT INTO note_tags(note_pk, tag)
             SELECT pk, 'Invalid' FROM notes",
        );
        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::InvalidStoredNote { field: "tag", .. })
        ));
    }

    #[test]
    fn invalid_persisted_storage_classes_include_safe_field_context() {
        let (repository, id) = corrupt_repository("UPDATE notes SET collection = X'736563726574'");

        let error = repository.get_note(&id).unwrap_err();
        let id_text = id.to_string();
        assert!(matches!(
            &error,
            NtError::InvalidStoredNote {
                context,
                field: "collection",
                source: Some(_),
            } if context.note_id.as_deref() == Some(id_text.as_str())
                && context.row_id == Some(1)
        ));
        assert_eq!(
            error.to_string(),
            format!("stored note is invalid (id: {id}, row: 1, field: collection)")
        );
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn invalid_tag_storage_classes_are_stored_note_errors_across_retrieval() {
        let (repository, id) = corrupt_repository(
            "INSERT INTO note_tags(note_pk, tag)
             SELECT pk, X'ff' FROM notes",
        );

        assert!(matches!(
            repository.get_note(&id),
            Err(NtError::InvalidStoredNote { field: "tag", .. })
        ));
        assert!(matches!(
            repository.list_tags(),
            Err(NtError::InvalidStoredNote { field: "tag", .. })
        ));
        assert_invalid_summary(&repository, "tag");
    }

    #[test]
    fn invalid_persisted_inventory_values_are_stored_note_errors() {
        let (repository, _) = corrupt_repository(
            "UPDATE notes SET collection = 'Invalid';
             INSERT INTO note_tags(note_pk, tag)
             SELECT pk, 'Invalid' FROM notes;",
        );

        assert!(matches!(
            repository.list_tags(),
            Err(NtError::InvalidStoredNote { field: "tag", .. })
        ));
        assert!(matches!(
            repository.list_collections(),
            Err(NtError::InvalidStoredNote {
                field: "collection",
                ..
            })
        ));
    }

    #[test]
    fn invalid_persisted_summary_values_are_stored_note_errors() {
        for (mutation, field) in [
            ("UPDATE notes SET id = 'malformed'", "id"),
            ("UPDATE notes SET updated = 'invalid'", "updated"),
            ("UPDATE notes SET collection = 'Invalid'", "collection"),
        ] {
            let (repository, _) = corrupt_repository(mutation);
            assert_invalid_summary(&repository, field);
        }

        let (repository, _) = corrupt_repository(
            "INSERT INTO note_tags(note_pk, tag)
             SELECT pk, 'Invalid' FROM notes",
        );
        assert_invalid_summary(&repository, "tag");
    }

    #[test]
    fn complete_note_load_uses_one_read_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nt.sqlite3");
        initialize_at(&path).unwrap();
        let mut writer = open_at(&path, OpenMode::ReadWrite).unwrap();
        let target = writer
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = writer
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Source")
                    .unwrap()
                    .with_tags(["old".parse().unwrap()])
                    .with_links([target.clone()]),
            )
            .unwrap();
        let mut reader = open_at(&path, OpenMode::ReadWrite).unwrap();
        let expected = reader.get_note(&source).unwrap();

        let transaction = reader
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .unwrap();
        transaction
            .query_row(
                "SELECT 1 FROM notes WHERE id = ?1",
                [source.to_string()],
                |_| Ok(()),
            )
            .unwrap();
        writer
            .change_tag(&source, AddOrRemove::Remove("old".parse().unwrap()))
            .unwrap();
        writer
            .change_tag(&source, AddOrRemove::Add("new".parse().unwrap()))
            .unwrap();
        writer.delete_notes(std::slice::from_ref(&target)).unwrap();

        assert_eq!(load_note(&transaction, &source).unwrap(), expected);
        transaction.commit().unwrap();
        assert_ne!(reader.get_note(&source).unwrap(), expected);
    }

    #[test]
    fn list_and_find_load_all_summary_tags() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# First\nbatched")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap(), "sqlite".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Second\nbatched")
                    .unwrap()
                    .with_tags(["cli".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Untagged\nbatched").unwrap())
            .unwrap();

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(notes.len(), 3);
        let first = notes.iter().find(|note| note.title() == "First").unwrap();
        assert_eq!(
            first.tags().iter().map(Tag::as_str).collect::<Vec<_>>(),
            ["rust", "sqlite"]
        );
        let untagged = notes
            .iter()
            .find(|note| note.title() == "Untagged")
            .unwrap();
        assert!(untagged.tags().is_empty());

        let query = NoteQuery::parse_find(&["batched".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query).len(), 3);
    }

    #[test]
    fn list_and_find_are_complete_by_default() {
        let mut repository = repository();
        for index in 0..1101 {
            repository
                .create_note(
                    NewNote::new(
                        CollectionPath::inbox(),
                        format!("# Note {index}\nshared limit term"),
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        let list = NoteQuery::parse_list(&[]).unwrap();
        assert_eq!(summaries(&repository, &list).len(), 1101);
        let find = NoteQuery::parse_find(&["shared".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &find).len(), 1101);

        let list = NoteQuery::parse_list(&["limit:7".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &list).len(), 7);
        let find = NoteQuery::parse_find(&["shared".to_string(), "limit:5".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &find).len(), 5);
    }

    #[test]
    fn summary_visiting_stops_before_later_rows_are_decoded() {
        let mut repository = repository();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Valid").unwrap())
            .unwrap();
        repository
            .connection
            .execute_batch(
                "PRAGMA ignore_check_constraints = ON;
                 INSERT INTO notes(id, collection, body, title, created, updated)
                 VALUES ('malformed', 'inbox', '# Invalid', 'Invalid',
                         '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z');
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .unwrap();

        let mut visited = 0;
        let result = repository.visit_note_summaries(&NoteQuery::default(), |_| {
            visited += 1;
            Err(NtError::Io(std::io::Error::other("stop visiting")))
        });

        assert!(matches!(result, Err(NtError::Io(_))));
        assert_eq!(visited, 1);
    }

    #[test]
    fn lists_current_tags_and_collections_once_in_lexical_order() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new("work/nt".parse().unwrap(), "# Work")
                    .unwrap()
                    .with_tags(["sqlite".parse().unwrap(), "rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Inbox")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();

        assert_eq!(
            repository
                .list_tags()
                .unwrap()
                .iter()
                .map(Tag::as_str)
                .collect::<Vec<_>>(),
            ["rust", "sqlite"]
        );
        assert_eq!(
            repository
                .list_collections()
                .unwrap()
                .iter()
                .map(CollectionPath::as_str)
                .collect::<Vec<_>>(),
            ["inbox", "work/nt"]
        );
    }

    #[test]
    fn summaries_count_outgoing_links() {
        let mut repository = repository();
        let first_target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First target").unwrap())
            .unwrap();
        let second_target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Second target").unwrap())
            .unwrap();
        let source = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Linked source")
                    .unwrap()
                    .with_links([first_target, second_target]),
            )
            .unwrap();

        let notes = summaries(&repository, &NoteQuery::default());
        assert_eq!(
            notes
                .iter()
                .find(|note| note.id() == &source)
                .unwrap()
                .outgoing(),
            2
        );
        assert!(
            notes
                .iter()
                .filter(|note| note.id() != &source)
                .all(|note| note.outgoing() == 0)
        );

        let query = NoteQuery::parse_find(&["linked source".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query)[0].outgoing(), 2);
    }

    #[test]
    fn validates_link_targets_and_atomic_deletion() {
        let mut repository = repository();
        let missing: NoteId = "018fbe0a-6c00-7000-8000-000000000001".parse().unwrap();
        let result = repository.create_note(
            NewNote::new(CollectionPath::inbox(), "# Link")
                .unwrap()
                .with_links([missing.clone()]),
        );
        assert!(matches!(result, Err(NtError::NoteNotFound(_))));

        let first = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First").unwrap())
            .unwrap();
        let result = repository.delete_notes(&[first.clone(), missing]);
        assert!(matches!(result, Err(NtError::NoteNotFound(_))));
        assert!(repository.get_note(&first).is_ok());
    }

    #[test]
    fn duplicate_deletion_is_rejected_without_deleting_the_note() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Kept").unwrap())
            .unwrap();

        let result = repository.delete_notes(&[id.clone(), id.clone()]);

        assert!(matches!(
            result,
            Err(NtError::DuplicateNoteId(duplicate)) if duplicate == id.to_string()
        ));
        assert!(repository.get_note(&id).is_ok());
    }

    #[test]
    fn list_filters_are_and_combined_and_negatable() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new("work/nt".parse().unwrap(), "# Rust")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Other").unwrap())
            .unwrap();
        let query = NoteQuery::parse_list(&[
            "collection:work/nt".to_string(),
            "not:tag:sqlite".to_string(),
        ])
        .unwrap();
        let notes = summaries(&repository, &query);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "Rust");
    }

    #[test]
    fn id_prefix_filters_preserve_matching_composition_and_ordering() {
        let repository = repository();
        let first: NoteId = "0198abcd-0000-7000-8000-000000000001".parse().unwrap();
        let second: NoteId = "0198abcd-0000-7000-8000-000000000002".parse().unwrap();
        let third: NoteId = "0198abcd-1000-7000-8000-000000000003".parse().unwrap();
        repository
            .connection
            .execute_batch(
                "INSERT INTO notes(id, collection, body, title, created, updated) VALUES
                     ('0198abcd-0000-7000-8000-000000000001', 'work/nt',
                      '# First\nSQLite prefix audit', 'First',
                      '2026-01-01T00:00:00Z', '2026-01-03T00:00:00Z'),
                     ('0198abcd-0000-7000-8000-000000000002', 'inbox',
                      '# Second\nOther prefix audit', 'Second',
                      '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z'),
                     ('0198abcd-1000-7000-8000-000000000003', 'work/nt',
                      '# Third\nSQLite prefix audit', 'Third',
                      '2026-01-01T00:00:00Z', '2026-01-03T00:00:00Z');
                 INSERT INTO note_tags(note_pk, tag)
                 SELECT pk, 'rust' FROM notes
                 WHERE id IN (
                     '0198abcd-0000-7000-8000-000000000001',
                     '0198abcd-1000-7000-8000-000000000003'
                 )",
            )
            .unwrap();

        let ids = |query: NoteQuery| {
            summaries(&repository, &query)
                .into_iter()
                .map(|summary| summary.id().clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            ids(NoteQuery::parse_list(&["id:0198".to_string()]).unwrap()),
            [third.clone(), first.clone(), second.clone()]
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&["id:0198abcd-0".to_string()]).unwrap()),
            [first.clone(), second]
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("id:{first}")]).unwrap()),
            std::slice::from_ref(&first)
        );
        assert!(ids(NoteQuery::parse_list(&["id:0199".to_string()]).unwrap()).is_empty());

        let tagged =
            NoteQuery::parse_list(&["id:0198".to_string(), "tag:rust".to_string()]).unwrap();
        assert_eq!(ids(tagged), [third.clone(), first.clone()]);

        let found = NoteQuery::parse_find(&["sqlite".to_string(), "id:0198".to_string()]).unwrap();
        assert_eq!(ids(found), [third, first]);
    }

    #[test]
    fn directional_link_filters_compose_and_preserve_order_and_limits() {
        let mut repository = repository();
        let b = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# B\nsqlite target")
                    .unwrap()
                    .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        let c = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# C\nother target").unwrap())
            .unwrap();
        let a = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# A\nsource")
                    .unwrap()
                    .with_links([b.clone(), c.clone()]),
            )
            .unwrap();
        let d = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# D\nsource")
                    .unwrap()
                    .with_links([b.clone()]),
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-01T00:00:00Z' WHERE id IN (?1, ?2)",
                params![b.to_string(), c.to_string()],
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
                params![a.to_string(), d.to_string()],
            )
            .unwrap();

        let ids = |query: NoteQuery| {
            summaries(&repository, &query)
                .into_iter()
                .map(|summary| summary.id().clone())
                .collect::<Vec<_>>()
        };
        let mut expected_sources = vec![a.clone(), d.clone()];
        expected_sources.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
        let mut expected_targets = vec![b.clone(), c.clone()];
        expected_targets.sort_by_key(|id| std::cmp::Reverse(id.to_string()));

        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("links-to:{b}")]).unwrap()),
            expected_sources
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{a}")]).unwrap()),
            expected_targets
        );
        assert_eq!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{d}")]).unwrap()),
            std::slice::from_ref(&b)
        );
        assert!(ids(NoteQuery::parse_list(&[format!("linked-from:{c}")]).unwrap()).is_empty());

        let tagged =
            NoteQuery::parse_list(&[format!("linked-from:{a}"), "tag:rust".to_string()]).unwrap();
        assert_eq!(ids(tagged), std::slice::from_ref(&b));

        let found =
            NoteQuery::parse_find(&["sqlite".to_string(), format!("linked-from:{a}")]).unwrap();
        assert_eq!(ids(found), std::slice::from_ref(&b));

        let excluded = NoteQuery::parse_list(&[format!("not:linked-from:{a}")]).unwrap();
        assert_eq!(
            ids(excluded).into_iter().collect::<BTreeSet<_>>(),
            [a.clone(), d.clone()].into_iter().collect()
        );

        let limited =
            NoteQuery::parse_list(&[format!("linked-from:{a}"), "limit:1".to_string()]).unwrap();
        assert_eq!(ids(limited), expected_targets[..1]);

        let missing = "018fbe0a-6c00-7000-8000-000000000001";
        assert!(
            ids(NoteQuery::parse_list(&[format!("linked-from:{missing}")]).unwrap()).is_empty()
        );
    }

    #[test]
    fn body_updates_detect_conflicts_but_metadata_does_not_create_them() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Original").unwrap())
            .unwrap();
        let mut note = repository.get_note(&id).unwrap();
        let expected = note.body_version();
        repository
            .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
            .unwrap();
        note.replace_body("# Edited", "2026-05-28T15:00:00Z".parse().unwrap())
            .unwrap();
        repository.replace_body(&note, expected).unwrap();
        assert_eq!(repository.get_note(&id).unwrap().body_version(), 2);

        let mut stale = repository.get_note(&id).unwrap();
        let stale_version = stale.body_version();
        repository
            .connection
            .execute(
                "UPDATE notes SET body_version = body_version + 1 WHERE id = ?1",
                [id.to_string()],
            )
            .unwrap();
        stale
            .replace_body("# Stale", "2026-05-28T16:00:00Z".parse().unwrap())
            .unwrap();
        assert!(matches!(
            repository.replace_body(&stale, stale_version),
            Err(NtError::ConcurrentEdit(_))
        ));
        assert!(matches!(
            repository.verify_body_version(&id, stale_version),
            Err(NtError::ConcurrentEdit(_))
        ));
    }

    #[test]
    fn metadata_changes_are_idempotent_and_touch_only_real_changes() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-01T00:00:00Z' WHERE id = ?1",
                [id.to_string()],
            )
            .unwrap();

        assert!(
            repository
                .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
                .unwrap()
        );
        let updated = repository.get_note(&id).unwrap().updated().clone();
        assert!(
            !repository
                .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
                .unwrap()
        );
        assert_eq!(repository.get_note(&id).unwrap().updated(), &updated);
        assert!(
            !repository
                .change_tag(&id, AddOrRemove::Remove("missing".parse().unwrap()))
                .unwrap()
        );
        assert!(
            repository
                .change_link(&id, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        assert!(
            !repository
                .change_link(&id, AddOrRemove::Add(target))
                .unwrap()
        );
        assert!(
            repository
                .move_note(&id, &"work/nt".parse().unwrap())
                .unwrap()
        );
        assert!(
            !repository
                .move_note(&id, &"work/nt".parse().unwrap())
                .unwrap()
        );
        let query = NoteQuery::parse_list(&["collection:work/nt".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &query).len(), 1);
    }

    #[test]
    fn link_changes_touch_only_the_source_when_the_edge_changes() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        let old = "2026-01-01T00:00:00Z";
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![old, source.to_string(), target.to_string()],
            )
            .unwrap();

        assert!(
            repository
                .change_link(&source, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        let source_updated = repository.get_note(&source).unwrap().updated().clone();
        assert_ne!(source_updated.as_str(), old);
        assert_eq!(
            repository.get_note(&target).unwrap().updated().as_str(),
            old
        );

        assert!(
            !repository
                .change_link(&source, AddOrRemove::Add(target.clone()))
                .unwrap()
        );
        assert_eq!(
            repository.get_note(&source).unwrap().updated(),
            &source_updated
        );
        assert_eq!(
            repository.get_note(&target).unwrap().updated().as_str(),
            old
        );

        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![old, source.to_string(), target.to_string()],
            )
            .unwrap();
        assert!(
            repository
                .change_link(&source, AddOrRemove::Remove(target.clone()))
                .unwrap()
        );
        let source_updated = repository.get_note(&source).unwrap().updated().clone();
        assert_ne!(source_updated.as_str(), old);
        assert_eq!(
            repository.get_note(&target).unwrap().updated().as_str(),
            old
        );

        assert!(
            !repository
                .change_link(&source, AddOrRemove::Remove(target.clone()))
                .unwrap()
        );
        assert_eq!(
            repository.get_note(&source).unwrap().updated(),
            &source_updated
        );
        assert_eq!(
            repository.get_note(&target).unwrap().updated().as_str(),
            old
        );
    }

    #[test]
    fn links_reject_self_references() {
        let mut repository = repository();
        let id = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
            .unwrap();
        assert!(matches!(
            repository.change_link(&id, AddOrRemove::Add(id.clone())),
            Err(NtError::SelfLink)
        ));
    }

    #[test]
    fn deleting_a_target_touches_surviving_sources_but_not_outgoing_targets() {
        let mut repository = repository();
        let target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
            .unwrap();
        let source = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Source")
                    .unwrap()
                    .with_links([target.clone()]),
            )
            .unwrap();
        let old = "2026-01-01T00:00:00Z";
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id IN (?2, ?3)",
                params![old, source.to_string(), target.to_string()],
            )
            .unwrap();
        repository
            .delete_notes(std::slice::from_ref(&target))
            .unwrap();
        let source_updated = repository.get_note(&source).unwrap().updated().clone();
        assert_ne!(source_updated.as_str(), old);
        assert!(
            !repository
                .change_link(&source, AddOrRemove::Remove(target))
                .unwrap()
        );
        assert_eq!(
            repository.get_note(&source).unwrap().updated(),
            &source_updated
        );

        let outgoing_target = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Outgoing target").unwrap())
            .unwrap();
        let deleted_source = repository
            .create_note(
                NewNote::new(CollectionPath::inbox(), "# Deleted source")
                    .unwrap()
                    .with_links([outgoing_target.clone()]),
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = ?1 WHERE id = ?2",
                params![old, outgoing_target.to_string()],
            )
            .unwrap();
        repository
            .delete_notes(std::slice::from_ref(&deleted_source))
            .unwrap();
        assert_eq!(
            repository
                .get_note(&outgoing_target)
                .unwrap()
                .updated()
                .as_str(),
            old
        );
    }

    #[test]
    fn find_uses_literal_complete_tokens_with_structured_filters() {
        let mut repository = repository();
        repository
            .create_note(
                NewNote::new(
                    "work/nt".parse().unwrap(),
                    "# Café storage\nOwnership and borrowing.",
                )
                .unwrap()
                .with_tags(["rust".parse().unwrap()]),
            )
            .unwrap();
        repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Storage shed").unwrap())
            .unwrap();

        let query =
            NoteQuery::parse_find(&["cafe ownership".to_string(), "tag:rust".to_string()]).unwrap();
        let notes = summaries(&repository, &query);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title(), "Café storage");

        let prefix = NoteQuery::parse_find(&["stor".to_string()]).unwrap();
        assert!(summaries(&repository, &prefix).is_empty());
        let punctuation = NoteQuery::parse_find(&["(storage*)".to_string()]).unwrap();
        assert_eq!(summaries(&repository, &punctuation).len(), 2);
    }

    #[test]
    fn list_find_and_limits_break_timestamp_ties_by_descending_id() {
        let mut repository = repository();
        let first = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# First\nordered").unwrap())
            .unwrap();
        let second = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Second\nordered").unwrap())
            .unwrap();
        let newest = repository
            .create_note(NewNote::new(CollectionPath::inbox(), "# Newest\nordered").unwrap())
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-02T00:00:00Z' WHERE id IN (?1, ?2)",
                params![first.to_string(), second.to_string()],
            )
            .unwrap();
        repository
            .connection
            .execute(
                "UPDATE notes SET updated = '2026-01-03T00:00:00Z' WHERE id = ?1",
                [newest.to_string()],
            )
            .unwrap();

        let mut tied = [first, second];
        tied.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
        let expected = vec![newest, tied[0].clone(), tied[1].clone()];
        for query in [
            NoteQuery::parse_list(&[]).unwrap(),
            NoteQuery::parse_find(&["ordered".to_string()]).unwrap(),
        ] {
            let actual = summaries(&repository, &query)
                .into_iter()
                .map(|summary| summary.id().clone())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }

        let limited =
            NoteQuery::parse_find(&["ordered".to_string(), "limit:2".to_string()]).unwrap();
        let actual = summaries(&repository, &limited)
            .into_iter()
            .map(|summary| summary.id().clone())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected[..2]);
    }
}

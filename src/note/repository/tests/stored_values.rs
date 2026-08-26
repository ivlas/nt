use super::super::super::{CollectionPath, NewNote, NoteId, NoteQuery, timestamp_now};
use super::super::Repository;
use super::repository;
use crate::error::NtError;

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
        repository.verify_body_version(&id, 1, None),
        Err(NtError::InvalidStoredNote {
            field: "body_version",
            ..
        })
    ));
    assert!(matches!(
        repository.replace_body(&changed_note, expected_version, None),
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

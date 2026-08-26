use rusqlite::params;

use super::super::super::{CollectionPath, NewNote, NoteQuery};
use super::super::AddOrRemove;
use super::{repository, summaries};
use crate::error::NtError;

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
fn revision_preconditions_guard_edits_metadata_noops_and_deleted_notes() {
    let mut repository = repository();
    let target = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
        .unwrap();
    let id = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Original").unwrap())
        .unwrap();
    let mut observed = repository.get_note(&id).unwrap();
    let observed_revision = observed.revision();
    let observed_body_version = observed.body_version();
    observed
        .replace_body("# Edited", "2026-05-28T15:00:00Z".parse().unwrap())
        .unwrap();

    repository
        .replace_body_if_revision(&observed, observed_body_version, Some(observed_revision))
        .unwrap();
    let edited = repository.get_note(&id).unwrap();
    let edited_revision = edited.revision();
    assert_eq!(edited.body(), "# Edited");

    for result in [
        repository.change_tag_if_revision(
            &id,
            AddOrRemove::Add("rust".parse().unwrap()),
            Some(observed_revision),
        ),
        repository.move_note_if_revision(&id, &"work/nt".parse().unwrap(), Some(observed_revision)),
        repository.change_link_if_revision(
            &id,
            AddOrRemove::Add(target.clone()),
            Some(observed_revision),
        ),
    ] {
        assert!(matches!(result, Err(NtError::RevisionConflict(_))));
    }
    let unchanged = repository.get_note(&id).unwrap();
    assert_eq!(unchanged.collection(), &CollectionPath::inbox());
    assert!(unchanged.tags().is_empty());
    assert!(unchanged.links().is_empty());
    assert_eq!(unchanged.revision(), edited_revision);

    repository
        .verify_body_version_if_revision(&id, unchanged.body_version(), Some(edited_revision))
        .unwrap();
    assert!(
        !repository
            .move_note_if_revision(&id, &CollectionPath::inbox(), Some(edited_revision))
            .unwrap()
    );
    assert_eq!(
        repository.get_note(&id).unwrap().revision(),
        edited_revision
    );

    let mut before_metadata = repository.get_note(&id).unwrap();
    repository
        .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()))
        .unwrap();
    let expected_version = before_metadata.body_version();
    let expected_revision = before_metadata.revision();
    before_metadata
        .replace_body(
            "# Stale after metadata",
            "2026-05-28T16:00:00Z".parse().unwrap(),
        )
        .unwrap();
    assert!(matches!(
        repository.replace_body_if_revision(
            &before_metadata,
            expected_version,
            Some(expected_revision),
        ),
        Err(NtError::RevisionConflict(_))
    ));

    let before_body = repository.get_note(&id).unwrap();
    let mut stale_edit = before_body.clone();
    let mut concurrent = before_body.clone();
    concurrent
        .replace_body("# Concurrent body", "2026-05-28T17:00:00Z".parse().unwrap())
        .unwrap();
    repository
        .replace_body(&concurrent, before_body.body_version())
        .unwrap();
    stale_edit
        .replace_body("# Stale body", "2026-05-28T18:00:00Z".parse().unwrap())
        .unwrap();
    assert!(matches!(
        repository.replace_body_if_revision(
            &stale_edit,
            before_body.body_version(),
            Some(before_body.revision()),
        ),
        Err(NtError::RevisionConflict(_))
    ));
    assert!(matches!(
        repository.change_tag_if_revision(
            &id,
            AddOrRemove::Add("stale".parse().unwrap()),
            Some(before_body.revision()),
        ),
        Err(NtError::RevisionConflict(_))
    ));

    let deleted = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Deleted").unwrap())
        .unwrap();
    let deleted_revision = repository.get_note(&deleted).unwrap().revision();
    repository
        .delete_notes(std::slice::from_ref(&deleted))
        .unwrap();
    assert!(matches!(
        repository.move_note_if_revision(
            &deleted,
            &"archive".parse().unwrap(),
            Some(deleted_revision),
        ),
        Err(NtError::NoteNotFound(_))
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

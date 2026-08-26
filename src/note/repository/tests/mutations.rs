use rusqlite::params;

use super::super::super::{CollectionPath, NewNote, NoteQuery};
use super::super::AddOrRemove;
use super::{repository, summaries};
use crate::error::NtError;

fn assert_revision_conflict(error: NtError, expected: u64, actual: u64) {
    assert!(matches!(
        error,
        NtError::RevisionConflict {
            expected: conflict_expected,
            actual: conflict_actual,
            ..
        } if conflict_expected == expected && conflict_actual == actual
    ));
}

#[test]
fn optimistic_revision_preconditions_guard_every_single_note_mutation() {
    let mut repository = repository();
    let target = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Target").unwrap())
        .unwrap();
    let id = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
        .unwrap();
    let observed = repository.get_note(&id).unwrap().revision();

    assert!(
        repository
            .change_tag(
                &id,
                AddOrRemove::Add("rust".parse().unwrap()),
                Some(observed),
            )
            .unwrap()
    );
    let after_tag = repository.get_note(&id).unwrap().revision();
    assert_revision_conflict(
        repository
            .change_tag(
                &id,
                AddOrRemove::Add("stale".parse().unwrap()),
                Some(observed),
            )
            .unwrap_err(),
        observed,
        after_tag,
    );
    assert_revision_conflict(
        repository
            .move_note(&id, &"work".parse().unwrap(), Some(observed))
            .unwrap_err(),
        observed,
        after_tag,
    );
    assert_revision_conflict(
        repository
            .change_link(&id, AddOrRemove::Add(target.clone()), Some(observed))
            .unwrap_err(),
        observed,
        after_tag,
    );

    let mut stale_body = repository.get_note(&id).unwrap();
    let stale_version = stale_body.body_version();
    stale_body
        .replace_body("# Edited", "2026-08-27T12:00:00Z".parse().unwrap())
        .unwrap();
    assert!(
        !repository
            .change_tag(
                &id,
                AddOrRemove::Add("rust".parse().unwrap()),
                Some(after_tag),
            )
            .unwrap()
    );
    assert_eq!(repository.get_note(&id).unwrap().revision(), after_tag);
    assert!(
        repository
            .move_note(&id, &"work".parse().unwrap(), Some(after_tag))
            .unwrap()
    );
    let after_move = repository.get_note(&id).unwrap().revision();
    assert_revision_conflict(
        repository
            .replace_body(&stale_body, stale_version, Some(after_tag))
            .unwrap_err(),
        after_tag,
        after_move,
    );

    let mut edited = repository.get_note(&id).unwrap();
    let expected_version = edited.body_version();
    edited
        .replace_body("# Edited", "2026-08-27T12:00:01Z".parse().unwrap())
        .unwrap();
    repository
        .replace_body(&edited, expected_version, Some(after_move))
        .unwrap();
    let after_edit = repository.get_note(&id).unwrap();
    assert_eq!(after_edit.body(), "# Edited");
    repository
        .verify_body_version(&id, after_edit.body_version(), Some(after_edit.revision()))
        .unwrap();
    assert_eq!(
        repository.get_note(&id).unwrap().revision(),
        after_edit.revision()
    );
    assert_revision_conflict(
        repository
            .change_tag(
                &id,
                AddOrRemove::Add("body-was-newer".parse().unwrap()),
                Some(after_move),
            )
            .unwrap_err(),
        after_move,
        after_edit.revision(),
    );

    let deleted = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Deleted").unwrap())
        .unwrap();
    let deleted_revision = repository.get_note(&deleted).unwrap().revision();
    repository
        .delete_notes(std::slice::from_ref(&deleted))
        .unwrap();
    assert!(matches!(
        repository.move_note(&deleted, &CollectionPath::inbox(), Some(deleted_revision),),
        Err(NtError::NoteNotFound(_))
    ));
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
        .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()), None)
        .unwrap();
    note.replace_body("# Edited", "2026-05-28T15:00:00Z".parse().unwrap())
        .unwrap();
    repository.replace_body(&note, expected, None).unwrap();
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
        repository.replace_body(&stale, stale_version, None),
        Err(NtError::ConcurrentEdit(_))
    ));
    assert!(matches!(
        repository.verify_body_version(&id, stale_version, None),
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
            .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()), None)
            .unwrap()
    );
    let updated = repository.get_note(&id).unwrap().updated().clone();
    assert!(
        !repository
            .change_tag(&id, AddOrRemove::Add("rust".parse().unwrap()), None)
            .unwrap()
    );
    assert_eq!(repository.get_note(&id).unwrap().updated(), &updated);
    assert!(
        !repository
            .change_tag(&id, AddOrRemove::Remove("missing".parse().unwrap()), None,)
            .unwrap()
    );
    assert!(
        repository
            .change_link(&id, AddOrRemove::Add(target.clone()), None)
            .unwrap()
    );
    assert!(
        !repository
            .change_link(&id, AddOrRemove::Add(target), None)
            .unwrap()
    );
    assert!(
        repository
            .move_note(&id, &"work/nt".parse().unwrap(), None)
            .unwrap()
    );
    assert!(
        !repository
            .move_note(&id, &"work/nt".parse().unwrap(), None)
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
            .change_link(&source, AddOrRemove::Add(target.clone()), None)
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
            .change_link(&source, AddOrRemove::Add(target.clone()), None)
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
            .change_link(&source, AddOrRemove::Remove(target.clone()), None)
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
            .change_link(&source, AddOrRemove::Remove(target.clone()), None)
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
        repository.change_link(&id, AddOrRemove::Add(id.clone()), None),
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
            .change_link(&source, AddOrRemove::Remove(target), None)
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

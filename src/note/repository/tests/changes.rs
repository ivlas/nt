use super::super::super::{CollectionPath, NewNote, timestamp_now};
use super::super::{AddOrRemove, Change, ChangeOperation, Repository};
use super::repository;

fn changes_since(repository: &Repository, revision: u64) -> Vec<Change> {
    let mut changes = Vec::new();
    repository
        .visit_changes_since(revision, |change| {
            changes.push(change);
            Ok(())
        })
        .unwrap();
    changes
}

#[test]
fn feed_records_canonical_operations_deletions_and_exact_cursor_boundaries() {
    let mut repository = repository();
    assert!(changes_since(&repository, 0).is_empty());

    let source = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Source").unwrap())
        .unwrap();
    let first_target = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# First target").unwrap())
        .unwrap();
    let second_target = repository
        .create_note(NewNote::new(CollectionPath::inbox(), "# Second target").unwrap())
        .unwrap();

    let mut edited = repository.get_note(&source).unwrap();
    edited
        .replace_body("# Edited", timestamp_now().unwrap())
        .unwrap();
    repository.replace_body(&edited, 1, None).unwrap();
    repository
        .change_tag(&source, AddOrRemove::Add("rust".parse().unwrap()), None)
        .unwrap();
    repository
        .change_link(&source, AddOrRemove::Add(first_target.clone()), None)
        .unwrap();
    repository
        .move_note(&source, &"work/nt".parse().unwrap(), None)
        .unwrap();

    assert!(
        !repository
            .change_tag(&source, AddOrRemove::Add("rust".parse().unwrap()), None)
            .unwrap()
    );
    assert!(
        !repository
            .change_link(&source, AddOrRemove::Add(first_target.clone()), None)
            .unwrap()
    );
    assert!(
        !repository
            .move_note(&source, &"work/nt".parse().unwrap(), None)
            .unwrap()
    );

    repository
        .change_link(&source, AddOrRemove::Add(second_target.clone()), None)
        .unwrap();
    repository
        .delete_notes(&[first_target.clone(), second_target.clone()])
        .unwrap();
    repository
        .delete_notes(std::slice::from_ref(&source))
        .unwrap();

    let changes = changes_since(&repository, 0);
    let mut expected_revision_nine = [
        (source.clone(), ChangeOperation::Metadata),
        (first_target.clone(), ChangeOperation::Remove),
        (second_target.clone(), ChangeOperation::Remove),
    ];
    expected_revision_nine.sort_by(|left, right| left.0.cmp(&right.0));
    let expected = [
        (1, source.clone(), ChangeOperation::Add),
        (2, first_target.clone(), ChangeOperation::Add),
        (3, second_target.clone(), ChangeOperation::Add),
        (4, source.clone(), ChangeOperation::Edit),
        (5, source.clone(), ChangeOperation::Metadata),
        (6, source.clone(), ChangeOperation::Metadata),
        (7, source.clone(), ChangeOperation::Metadata),
        (8, source.clone(), ChangeOperation::Metadata),
    ]
    .into_iter()
    .chain(
        expected_revision_nine
            .into_iter()
            .map(|(id, operation)| (9, id, operation)),
    )
    .chain([(10, source.clone(), ChangeOperation::Remove)])
    .collect::<Vec<_>>();
    assert_eq!(
        changes
            .iter()
            .map(|change| (
                change.revision(),
                change.note_id().clone(),
                change.operation()
            ))
            .collect::<Vec<_>>(),
        expected
    );

    assert_eq!(
        changes_since(&repository, 9)
            .iter()
            .map(|change| (change.revision(), change.note_id(), change.operation()))
            .collect::<Vec<_>>(),
        [(10, &source, ChangeOperation::Remove)]
    );
    assert!(changes_since(&repository, 10).is_empty());
}

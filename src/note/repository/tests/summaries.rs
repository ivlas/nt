use super::super::super::{CollectionPath, NewNote, NoteQuery, Tag};
use super::{repository, summaries};
use crate::error::NtError;

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

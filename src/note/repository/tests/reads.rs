use std::io;

use super::super::super::{CollectionPath, NewNote, NoteQuery};
use super::repository;
use crate::error::NtError;

#[test]
fn full_note_visiting_stops_before_later_rows_are_decoded() {
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
    let error = repository
        .visit_notes(&NoteQuery::default(), |_| {
            visited += 1;
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed").into())
        })
        .unwrap_err();

    assert_eq!(visited, 1);
    assert!(matches!(error, NtError::Io(error) if error.kind() == io::ErrorKind::BrokenPipe));
}

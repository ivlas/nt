use std::io;

use rusqlite::params;

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
             INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
             VALUES ('malformed', 'inbox', '# Invalid', 'Invalid',
                      '2000-01-01T00:00:00Z', '2000-01-01T00:00:00Z', 1);
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

#[test]
fn batch_read_scales_to_thousands_without_one_parameter_per_id() {
    let mut repository = repository();
    let transaction = repository.connection.transaction().unwrap();
    {
        let mut insert = transaction
            .prepare(
                "INSERT INTO notes(id, collection, body, title, created, updated, note_revision)
                 VALUES (?1, 'inbox', ?2, ?3, '2026-01-01T00:00:00Z',
                         '2026-01-01T00:00:00Z', 1)",
            )
            .unwrap();
        for index in 0..2_000 {
            insert
                .execute(params![
                    format!("018fbe0a-6c00-7000-8000-{index:012x}"),
                    format!("# Note {index}\nBody {index}"),
                    format!("Note {index}"),
                ])
                .unwrap();
        }
    }
    transaction.commit().unwrap();

    let mut expressions = (0..2_000)
        .rev()
        .map(|index| format!("id:018fbe0a-6c00-7000-8000-{index:012x}"))
        .collect::<Vec<_>>();
    expressions.push(expressions[0].clone());
    expressions.push("id:018fbe0a-6c00-7000-8000-ffffffffffff".to_string());
    let query = NoteQuery::parse_read(&expressions).unwrap();
    let mut notes = Vec::new();
    repository
        .visit_notes(&query, |note| {
            notes.push((note.id().to_string(), note.body().to_string()));
            Ok(())
        })
        .unwrap();

    assert_eq!(notes.len(), 2_000);
    assert_eq!(
        notes.first().unwrap(),
        &(
            "018fbe0a-6c00-7000-8000-0000000007cf".to_string(),
            "# Note 1999\nBody 1999".to_string()
        )
    );
    assert_eq!(
        notes.last().unwrap(),
        &(
            "018fbe0a-6c00-7000-8000-000000000000".to_string(),
            "# Note 0\nBody 0".to_string()
        )
    );
}

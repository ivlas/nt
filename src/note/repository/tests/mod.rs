use super::super::NoteQuery;
use super::{NoteSummary, Repository};
use crate::schema;

mod changes;
mod lifecycle;
mod mutations;
mod queries;
mod reads;
mod revisions;
mod stored_values;
mod summaries;

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

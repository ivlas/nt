use std::collections::BTreeSet;

use crate::error::{NtError, Result};
use crate::note::{NoteId, Repository};
use crate::schema;

use super::{App, stdin_ids, write_commit_output};

pub(super) fn rm(app: &mut App<'_>, ids: &[String]) -> Result<()> {
    if ids == [stdin_ids::STDIN_IDS] {
        let parsed = stdin_ids::parse(app)?;
        return remove(app, &parsed);
    }
    let mut parsed = Vec::with_capacity(ids.len());
    let mut unique = BTreeSet::new();
    for value in ids {
        let id: NoteId = value.parse()?;
        if !unique.insert(id.clone()) {
            return Err(NtError::DuplicateNoteId(value.clone()));
        }
        parsed.push(id);
    }
    remove(app, &parsed)
}

fn remove(app: &mut App<'_>, ids: &[NoteId]) -> Result<()> {
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.delete_notes(ids)?;
    write_commit_output(app.output, format_args!("removed {}\n", ids.len()))
}

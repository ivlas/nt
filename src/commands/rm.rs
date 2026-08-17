use std::collections::BTreeSet;

use crate::domains::note::{NoteId, Repository};
use crate::error::{NtError, Result};

use super::{App, write_commit_output};

pub(super) fn rm(app: &mut App<'_>, ids: &[String]) -> Result<()> {
    let mut parsed = Vec::with_capacity(ids.len());
    let mut unique = BTreeSet::new();
    for value in ids {
        let id: NoteId = value.parse()?;
        if !unique.insert(id.clone()) {
            return Err(NtError::DuplicateNoteId(value.clone()));
        }
        parsed.push(id);
    }
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.delete_notes(&parsed)?;
    write_commit_output(app.output, format_args!("removed {}\n", parsed.len()))?;
    Ok(())
}

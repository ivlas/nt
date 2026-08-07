use std::io::{self, Write};

use crate::error::Result;
use crate::note::NoteId;
use crate::repository::Repository;

pub(super) fn show(id: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let mut repository = Repository::open()?;
    let note = repository.get_note(&id)?;
    io::stdout().write_all(note.body().as_bytes())?;
    Ok(())
}

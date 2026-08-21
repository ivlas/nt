use crate::domains::library::LibraryItemId;
use crate::domains::note::NoteId;
use crate::error::Result;
use crate::relations::NoteLibraryRepository;

use super::{App, write_commit_output};

pub(super) fn reference(app: &mut App<'_>, note_id: &str, library_id: &str) -> Result<()> {
    let note_id: NoteId = note_id.parse()?;
    let library_id: LibraryItemId = library_id.parse()?;
    let mut repository = NoteLibraryRepository::open_at(app.database_path()?)?;
    repository.reference(&note_id, &library_id)?;
    write_commit_output(
        app.output,
        format_args!("referenced {note_id} {library_id}\n"),
    )
}

pub(super) fn unreference(app: &mut App<'_>, note_id: &str, library_id: &str) -> Result<()> {
    let note_id: NoteId = note_id.parse()?;
    let library_id: LibraryItemId = library_id.parse()?;
    let mut repository = NoteLibraryRepository::open_at(app.database_path()?)?;
    repository.unreference(&note_id, &library_id)?;
    write_commit_output(
        app.output,
        format_args!("unreferenced {note_id} {library_id}\n"),
    )
}

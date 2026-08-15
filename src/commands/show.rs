use crate::error::Result;
use crate::note::NoteId;
use crate::repository::Repository;

use super::App;

pub(super) fn show(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let mut repository = Repository::open_read_only(app.database_path()?)?;
    let note = repository.get_note(&id)?;
    app.output.write_all(note.body().as_bytes())?;
    Ok(())
}

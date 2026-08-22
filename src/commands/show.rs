use crate::error::Result;
use crate::note::{NoteId, Repository};
use crate::schema;

use super::App;

pub(super) fn show(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    let note = repository.get_note(&id)?;
    app.output.write_all(note.body().as_bytes())?;
    app.output.flush()?;
    Ok(())
}

use crate::error::Result;
use crate::note::{CollectionPath, NoteId};
use crate::repository::Repository;

use super::App;

pub(super) fn move_note(app: &mut App<'_>, id: &str, collection: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let collection: CollectionPath = collection.parse()?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.move_note(&id, &collection)?;
    writeln!(app.output, "moved {id} {collection}")?;
    Ok(())
}

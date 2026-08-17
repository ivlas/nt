use crate::domains::note::{CollectionPath, NoteId, Repository};
use crate::error::Result;

use super::{App, write_commit_output};

pub(super) fn move_note(app: &mut App<'_>, id: &str, collection: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let collection: CollectionPath = collection.parse()?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.move_note(&id, &collection)?;
    write_commit_output(app.output, format_args!("moved {id} {collection}\n"))?;
    Ok(())
}

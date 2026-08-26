use crate::error::Result;
use crate::note::{CollectionPath, NoteId, Repository};
use crate::schema;

use super::{App, parse_if_revision, stdin_ids, write_commit_output};

pub(super) fn move_note(
    app: &mut App<'_>,
    id: &str,
    collection: &str,
    if_revision: Option<&str>,
) -> Result<()> {
    let collection: CollectionPath = collection.parse()?;
    if id == stdin_ids::STDIN_IDS {
        stdin_ids::reject_precondition(if_revision)?;
        let ids = stdin_ids::parse(app)?;
        let mut repository =
            Repository::from_connection(schema::open_read_write(app.database_path()?)?);
        repository.move_notes(&ids, &collection)?;
        write_commit_output(
            app.output,
            format_args!("moved {} {collection}\n", ids.len()),
        )?;
        return Ok(());
    }
    let id: NoteId = id.parse()?;
    let if_revision = parse_if_revision(if_revision)?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.move_note(&id, &collection, if_revision)?;
    write_commit_output(app.output, format_args!("moved {id} {collection}\n"))?;
    Ok(())
}

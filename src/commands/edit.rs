use crate::error::Result;
use crate::note::{NoteId, Repository, timestamp_now};
use crate::schema;

use super::{App, write_commit_output};

pub(super) fn edit(app: &mut App<'_>, id: &str, body_arguments: &[String]) -> Result<()> {
    let id: NoteId = id.parse()?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    let mut note = repository.get_note(&id)?;
    let expected_version = note.body_version();
    let body = app.input.read_body(body_arguments, Some(note.body()))?;
    if note.replace_body(body, timestamp_now()?)? {
        repository.replace_body(&note, expected_version)?;
    } else {
        repository.verify_body_version(&id, expected_version)?;
    }
    write_commit_output(app.output, format_args!("updated {id}\n"))?;
    Ok(())
}

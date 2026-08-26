use crate::error::Result;
use crate::note::{NoteId, Repository, timestamp_now};
use crate::schema;

use super::{App, parse_revision_precondition, write_commit_output};

pub(super) fn edit(
    app: &mut App<'_>,
    id: &str,
    precondition: Option<&str>,
    body_arguments: &[String],
) -> Result<()> {
    let id: NoteId = id.parse()?;
    let expected_revision = parse_revision_precondition(precondition)?;
    let mut note =
        Repository::from_connection(schema::open_read_only(app.database_path()?)?).get_note(&id)?;
    let expected_version = note.body_version();
    let body = app.input.read_body(body_arguments, Some(note.body()))?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    if note.replace_body(body, timestamp_now()?)? {
        repository.replace_body_if_revision(&note, expected_version, expected_revision)?;
    } else {
        repository.verify_body_version_if_revision(&id, expected_version, expected_revision)?;
    }
    write_commit_output(app.output, format_args!("updated {id}\n"))?;
    Ok(())
}

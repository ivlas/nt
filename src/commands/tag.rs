use crate::error::Result;
use crate::note::{NoteId, Repository, Tag};
use crate::schema;

use super::{App, parse_add_or_remove, parse_revision_precondition, write_commit_output};

pub(super) fn tag(
    app: &mut App<'_>,
    id: &str,
    operation: &str,
    precondition: Option<&str>,
) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_add_or_remove::<Tag>(operation, "tag operation")?;
    let expected_revision = parse_revision_precondition(precondition)?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.change_tag_if_revision(&id, operation.clone(), expected_revision)?;
    write_commit_output(app.output, format_args!("tagged {id} {operation}\n"))?;
    Ok(())
}

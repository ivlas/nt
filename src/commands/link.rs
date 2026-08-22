use crate::error::Result;
use crate::note::{NoteId, Repository};
use crate::schema;

use super::{App, parse_add_or_remove, write_commit_output};

pub(super) fn link(app: &mut App<'_>, id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_add_or_remove::<NoteId>(operation, "link operation")?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.change_link(&id, operation.clone())?;
    write_commit_output(app.output, format_args!("linked {id} {operation}\n"))?;
    Ok(())
}

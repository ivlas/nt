use crate::error::Result;
use crate::note::{NoteId, Tag};
use crate::repository::Repository;

use super::{App, parse_add_or_remove};

pub(super) fn tag(app: &mut App<'_>, id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_add_or_remove::<Tag>(operation, "tag operation")?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.change_tag(&id, operation.clone())?;
    writeln!(app.output, "tagged {id} {operation}")?;
    Ok(())
}

use crate::error::Result;
use crate::note::NoteId;
use crate::repository::Repository;

use super::{App, parse_add_or_remove};

pub(super) fn link(app: &mut App<'_>, id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_add_or_remove::<NoteId>(operation, "link operation")?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.change_link(&id, operation.clone())?;
    writeln!(app.output, "linked {id} {operation}")?;
    Ok(())
}

use crate::error::{NtError, Result};
use crate::note::NoteId;
use crate::repository::{AddOrRemove, Repository};

use super::App;

pub(super) fn link(app: &mut App<'_>, id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_operation(operation)?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    repository.change_link(&id, operation.clone())?;
    writeln!(app.output, "linked {id} {operation}")?;
    Ok(())
}

fn parse_operation(value: &str) -> Result<AddOrRemove<NoteId>> {
    match value.as_bytes().first() {
        Some(b'+') => Ok(AddOrRemove::Add(value[1..].parse()?)),
        Some(b'-') => Ok(AddOrRemove::Remove(value[1..].parse()?)),
        _ => Err(NtError::InvalidValue {
            field: "link operation",
            value: value.to_string(),
        }),
    }
}

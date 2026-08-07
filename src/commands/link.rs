use crate::error::{NtError, Result};
use crate::note::NoteId;
use crate::repository::{AddOrRemove, Repository};

pub(super) fn link(id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_operation(operation)?;
    let mut repository = Repository::open()?;
    repository.change_link(&id, operation.clone())?;
    println!("linked {id} {operation}");
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

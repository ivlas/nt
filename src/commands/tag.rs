use crate::error::{NtError, Result};
use crate::note::{NoteId, Tag};
use crate::repository::{AddOrRemove, Repository};

pub(super) fn tag(id: &str, operation: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let operation = parse_operation(operation)?;
    let mut repository = Repository::open()?;
    repository.change_tag(&id, operation.clone())?;
    println!("tagged {id} {operation}");
    Ok(())
}

fn parse_operation(value: &str) -> Result<AddOrRemove<Tag>> {
    match value.as_bytes().first() {
        Some(b'+') => Ok(AddOrRemove::Add(value[1..].parse()?)),
        Some(b'-') => Ok(AddOrRemove::Remove(value[1..].parse()?)),
        _ => Err(NtError::InvalidValue {
            field: "tag operation",
            value: value.to_string(),
        }),
    }
}

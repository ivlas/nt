use crate::error::{NtError, Result};
use crate::note::validate_id;
use crate::repository::Repository;
use std::collections::BTreeSet;

pub(super) fn rm(ids: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();

    for id in ids {
        validate_id(id)?;
        if !seen.insert(id.as_str()) {
            return Err(NtError::Message(format!("duplicate note id: {id}")));
        }
    }

    let mut repository = Repository::open()?;
    repository.delete_notes(ids)?;

    for id in ids {
        println!("removed {id}");
    }
    Ok(())
}

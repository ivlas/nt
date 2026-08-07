use crate::error::Result;
use crate::note::{CollectionPath, NoteId};
use crate::repository::Repository;

pub(super) fn move_note(id: &str, collection: &str) -> Result<()> {
    let id: NoteId = id.parse()?;
    let collection: CollectionPath = collection.parse()?;
    let mut repository = Repository::open()?;
    repository.move_note(&id, &collection)?;
    println!("moved {id} {collection}");
    Ok(())
}

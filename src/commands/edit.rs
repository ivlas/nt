use crate::error::Result;
use crate::input::read_body;
use crate::note::{NoteId, timestamp_now};
use crate::repository::Repository;

pub(super) fn edit(id: &str, body_arguments: &[String]) -> Result<()> {
    let id: NoteId = id.parse()?;
    let mut repository = Repository::open()?;
    let mut note = repository.get_note(&id)?;
    let expected_version = note.body_version();
    let body = read_body(body_arguments, Some(note.body()))?;
    if note.replace_body(body, timestamp_now())? {
        repository.replace_body(&note, expected_version)?;
    } else {
        repository.verify_body_version(&id, expected_version)?;
    }
    println!("updated {id}");
    Ok(())
}

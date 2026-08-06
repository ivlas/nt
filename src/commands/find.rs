use crate::error::Result;
use crate::query::NoteQuery;
use crate::repository::Repository;

pub(super) fn find(expressions: &[String]) -> Result<()> {
    let query = NoteQuery::parse_find(expressions)?;
    let repository = Repository::open()?;
    super::list::print_notes(repository.find_notes(&query)?)
}

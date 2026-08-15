use crate::error::Result;
use crate::query::NoteQuery;
use crate::repository::Repository;

use super::App;

pub(super) fn find(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = NoteQuery::parse_find(expressions)?;
    let repository = Repository::open_read_only(app.database_path()?)?;
    super::list::print_notes(&repository, &query, app.output, app.output_is_terminal)
}

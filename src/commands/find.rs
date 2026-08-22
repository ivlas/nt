use crate::error::Result;
use crate::note::{NoteQuery, Repository};
use crate::schema;

use super::App;

pub(super) fn find(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = NoteQuery::parse_find(expressions)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    crate::cli::rendering::print_notes(&repository, &query, app.output, app.output_is_terminal)
}

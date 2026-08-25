use crate::cli::rendering::print_full_notes;
use crate::error::Result;
use crate::note::{NoteQuery, Repository};
use crate::schema;

use super::App;

pub(super) fn read(app: &mut App<'_>, filters: &[String]) -> Result<()> {
    let query = NoteQuery::parse_read(filters)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    print_full_notes(&repository, &query, app.output, app.output_is_terminal)
}

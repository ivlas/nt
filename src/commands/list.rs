use crate::cli::rendering::{print_notes, print_values};
use crate::domains::note::{NoteQuery, Repository};
use crate::error::Result;

use super::App;

pub(super) fn list(app: &mut App<'_>, arguments: &[String]) -> Result<()> {
    match arguments {
        [target] if target == "tags" => {
            let repository = Repository::open_read_only(app.database_path()?)?;
            print_values(
                app.output,
                app.output_is_terminal,
                "tag",
                repository.list_tags()?,
            )
        }
        [target] if target == "collections" => {
            let repository = Repository::open_read_only(app.database_path()?)?;
            print_values(
                app.output,
                app.output_is_terminal,
                "collection",
                repository.list_collections()?,
            )
        }
        filters => {
            let query = NoteQuery::parse_list(filters)?;
            let repository = Repository::open_read_only(app.database_path()?)?;
            print_notes(&repository, &query, app.output, app.output_is_terminal)
        }
    }
}

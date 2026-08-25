use crate::cli::rendering::print_full_notes;
use crate::error::{NtError, Result};
use crate::note::{NoteId, NoteQuery, Repository};
use crate::schema;

use super::App;

pub(super) fn read(app: &mut App<'_>, filters: &[String]) -> Result<()> {
    let query = parse_query(app, filters)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    print_full_notes(&repository, &query, app.output, app.output_is_terminal)
}

fn parse_query(app: &mut App<'_>, filters: &[String]) -> Result<NoteQuery> {
    const STDIN_IDS: &str = "id:-";

    let marker_count = filters
        .iter()
        .filter(|filter| filter.as_str() == STDIN_IDS)
        .count();
    if marker_count == 0 {
        return NoteQuery::parse_read(filters);
    }
    if marker_count > 1 {
        return Err(NtError::InvalidValue {
            field: "filter",
            value: "duplicate id:-".to_string(),
        });
    }

    let mut expressions = filters
        .iter()
        .filter(|filter| filter.as_str() != STDIN_IDS)
        .cloned()
        .collect::<Vec<_>>();
    let input = app.input.read_stdin()?;
    let original_len = expressions.len();
    for line in input.lines() {
        let id: NoteId = line.parse()?;
        expressions.push(format!("id:{id}"));
    }
    if expressions.len() == original_len {
        return Err(NtError::InvalidValue {
            field: "stdin IDs",
            value: "empty".to_string(),
        });
    }
    NoteQuery::parse_read(&expressions)
}

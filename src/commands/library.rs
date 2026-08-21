use crate::cli::rendering::{print_library_history, print_library_summaries};
use crate::domains::library::{
    LibraryItemId, LibraryQuery, NewLibraryCapture, NewLibraryItem, Repository,
};
use crate::error::Result;

use super::{App, write_commit_output};

pub(super) fn add(app: &mut App<'_>, source: &str, title: &[String]) -> Result<()> {
    let mut repository = Repository::open_at(app.database_path()?)?;
    let content = app.input.read_body(&[], None)?;
    let item = NewLibraryItem::new(source, title.join(" "), content)?;
    let outcome = repository.create_item(item)?;
    let action = if outcome.item_created() {
        "saved"
    } else if outcome.capture_created() {
        "captured"
    } else {
        "unchanged"
    };
    write_commit_output(
        app.output,
        format_args!("library {action} {}\n", outcome.id()),
    )
}

pub(super) fn capture(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: LibraryItemId = id.parse()?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    let content = app.input.read_body(&[], None)?;
    let capture = NewLibraryCapture::new(content)?;
    repository.capture(&id, capture)?;
    write_commit_output(app.output, format_args!("captured {id}\n"))
}

pub(super) fn show(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: LibraryItemId = id.parse()?;
    let repository = Repository::open_read_only(app.database_path()?)?;
    let capture = repository.get_latest_capture(&id)?;
    app.output.write_all(capture.content().as_bytes())?;
    Ok(())
}

pub(super) fn find(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = LibraryQuery::parse_find(expressions)?;
    let repository = Repository::open_read_only(app.database_path()?)?;
    print_library_summaries(&repository, &query, app.output, app.output_is_terminal)
}

pub(super) fn summary(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: LibraryItemId = id.parse()?;
    let mut repository = Repository::open_at(app.database_path()?)?;
    let summary = app.input.read_body(&[], None)?;
    repository.replace_latest_summary(&id, &summary, "manual", "1")?;
    write_commit_output(app.output, format_args!("summarized {id}\n"))
}

pub(super) fn history(app: &mut App<'_>, id: &str) -> Result<()> {
    let id: LibraryItemId = id.parse()?;
    let repository = Repository::open_read_only(app.database_path()?)?;
    let history = repository.history(&id)?;
    print_library_history(&history, app.output, app.output_is_terminal)
}

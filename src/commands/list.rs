use std::io::{self, IsTerminal};

use crate::error::Result;
use crate::listing::{ListRequest, render_row, render_table};
use crate::repository::Repository;

pub(super) fn list(args: &[String]) -> Result<()> {
    let repository = Repository::open()?;
    let ListRequest { fields, filters } = ListRequest::parse(args)?;
    let rows = repository.list_rows(&fields, &filters)?;

    if io::stdout().is_terminal() {
        for line in render_table(&rows, &fields) {
            println!("{line}");
        }
    } else {
        for row in &rows {
            println!("{}", render_row(row));
        }
    }
    Ok(())
}

use std::io::{self, IsTerminal};

use crate::error::Result;
use crate::listing::{ListRequest, render_row, render_table};
use crate::query::Query;
use crate::repository::{NoteMeta, Repository};

pub(super) fn list(args: &[String]) -> Result<()> {
    let repository = Repository::open()?;
    let ListRequest { fields, query } = ListRequest::parse(args)?;
    let candidates = repository.list_notes()?;
    let notes = matching_notes(&candidates, &query)?;

    if io::stdout().is_terminal() {
        for line in render_table(&notes, &fields) {
            println!("{line}");
        }
    } else {
        for note in notes {
            println!("{}", render_row(note, &fields));
        }
    }
    Ok(())
}

fn matching_notes<'a>(notes: &'a [NoteMeta], query: &Query) -> Result<Vec<&'a NoteMeta>> {
    let mut matching = Vec::new();
    for note in notes {
        if query.matches(note)? {
            matching.push(note);
        }
    }
    Ok(matching)
}

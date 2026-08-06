use std::io::{self, IsTerminal};

use crate::error::Result;
use crate::query::NoteQuery;
use crate::repository::{NoteSummary, Repository};

pub(super) fn list(filters: &[String]) -> Result<()> {
    let query = NoteQuery::parse_list(filters)?;
    let repository = Repository::open()?;
    print_notes(repository.list_notes(&query)?)
}

pub(super) fn print_notes(notes: Vec<NoteSummary>) -> Result<()> {
    let tty = io::stdout().is_terminal();
    for note in notes {
        if tty {
            print_tty(&note);
        } else {
            print_redirected(&note)?;
        }
    }
    Ok(())
}

fn print_tty(note: &NoteSummary) {
    let tags = note
        .tags()
        .iter()
        .map(|tag| tag.as_str())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{}\t{}\t{}\t{}\t{}",
        note.id(),
        note.updated(),
        note.collection(),
        note.title(),
        tags
    );
}

fn print_redirected(note: &NoteSummary) -> Result<()> {
    let tags = note
        .tags()
        .iter()
        .map(|tag| tag.as_str())
        .collect::<Vec<_>>();
    println!(
        "{}\t{}\t{}\t{}\t{}",
        serde_json::to_string(&note.id().to_string())?,
        serde_json::to_string(note.updated().as_str())?,
        serde_json::to_string(note.collection().as_str())?,
        serde_json::to_string(note.title())?,
        serde_json::to_string(&tags)?,
    );
    Ok(())
}

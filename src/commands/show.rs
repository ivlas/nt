use crate::display::{find_summary_line, joined_or_dash};
use crate::error::Result;
use crate::note::{NoteId, NoteKind};
use crate::query::Query;
use crate::repository::Repository;
use crate::terminal::{Style, paint};

pub(super) fn show(id: &str) -> Result<()> {
    let text = show_text_for_display(id, crate::terminal::stdout_color_enabled())?;

    print!("{text}");
    if !text.ends_with('\n') {
        println!();
    }

    Ok(())
}

fn show_text_for_display(id: &str, color: bool) -> Result<String> {
    let id: NoteId = id.parse()?;
    let repository = Repository::open()?;
    let note = repository.get_note(&id)?;

    let mut text = String::new();
    text.push_str(&format!(
        "{}  {}\n",
        paint(note.id.as_str(), Style::BrightCyan, color),
        note.title
    ));
    text.push_str(&format!("home {}\n", note.home_collection));
    text.push_str(&format!(
        "created {}\n",
        paint(&note.created, Style::Dim, color)
    ));
    text.push_str(&format!(
        "updated {}\n",
        paint(&note.updated, Style::Dim, color)
    ));
    text.push_str(&format!("kind {}\n", note.kind));
    if note.kind == NoteKind::Todo {
        text.push_str(&format!(
            "status {}\n",
            note.status.map(|value| value.as_str()).unwrap_or("-")
        ));
        text.push_str(&format!(
            "priority {}\n",
            note.priority.map(|value| value.as_str()).unwrap_or("-")
        ));
        text.push_str(&format!(
            "scheduled {}\n",
            note.scheduled.as_deref().unwrap_or("-")
        ));
        text.push_str(&format!("due {}\n", note.due.as_deref().unwrap_or("-")));
        text.push_str(&format!(
            "closed {}\n",
            note.closed.as_deref().unwrap_or("-")
        ));
    }
    text.push_str(&format!(
        "tags {}\n",
        paint(&joined_or_dash(&note.tags), Style::Green, color)
    ));
    text.push_str(&format!(
        "collections {}\n",
        joined_or_dash(&note.collections)
    ));
    text.push_str(&format!("links {}\n", joined_or_dash(&note.links)));
    text.push_str(&format!("sources {}\n\n", joined_or_dash(&note.sources)));
    text.push_str(&note.body);
    if !text.ends_with('\n') {
        text.push('\n');
    }

    Ok(text)
}

pub(super) fn find(exprs: &[String]) -> Result<()> {
    let query = Query::parse(exprs)?;
    let repository = Repository::open()?;

    for note in repository.find_rows(&query)? {
        println!("{}", find_summary_line(&note));
    }

    Ok(())
}

use std::collections::BTreeSet;
use std::io::{self, IsTerminal};

use crate::display::summary_line_for_display;
use crate::error::Result;
use crate::index::{Index, NoteMeta};
use crate::listing::{ListRequest, render_link_row, render_link_table, render_row, render_table};
use crate::query::Query;

use super::{note_ref, validate_collection, validate_source, validate_tag};

pub(super) fn list(args: &[String]) -> Result<()> {
    let index = Index::load()?;
    match ListRequest::parse(args)? {
        ListRequest::Notes { fields, query } => {
            let notes = matching_notes(&index, &query)?;

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
        ListRequest::Tags(tag) => {
            list_metadata(&index, tag.as_deref(), validate_tag, |note| &note.tags)
        }
        ListRequest::Collections(collection) => {
            list_metadata(&index, collection.as_deref(), validate_collection, |note| {
                &note.collections
            })
        }
        ListRequest::Sources(source) => {
            list_metadata(&index, source.as_deref(), validate_source, |note| {
                &note.sources
            })
        }
        ListRequest::LinkGraph { query, from, to } => {
            list_link_graph(&index, &query, from.as_deref(), to.as_deref())
        }
    }
}

fn matching_notes<'a>(index: &'a Index, query: &Query) -> Result<Vec<&'a NoteMeta>> {
    let mut notes = Vec::new();
    for note in index.active_notes() {
        if query.matches(note)? {
            notes.push(note);
        }
    }
    Ok(notes)
}

fn list_link_graph(
    index: &Index,
    query: &Query,
    from_id: Option<&str>,
    to_id: Option<&str>,
) -> Result<()> {
    let links = matching_notes(index, query)?
        .into_iter()
        .filter(|note| from_id.is_none_or(|id| note.id == id))
        .flat_map(move |from| {
            from.links
                .iter()
                .filter(move |id| to_id.is_none_or(|selected| id.as_str() == selected))
                .filter_map(move |id| note_ref(index, id).ok().map(|to| (from, to)))
        })
        .collect::<Vec<_>>();

    if io::stdout().is_terminal() {
        for line in render_link_table(&links) {
            println!("{line}");
        }
    } else {
        for (from, to) in links {
            println!("{}", render_link_row(from, to));
        }
    }

    Ok(())
}

fn list_metadata<'a>(
    index: &'a Index,
    selected: Option<&str>,
    validate: impl Fn(&str) -> Result<()>,
    values: impl Fn(&'a NoteMeta) -> &'a [String],
) -> Result<()> {
    if let Some(selected) = selected {
        validate(selected)?;
        return print_note_list(
            index
                .active_notes()
                .into_iter()
                .filter(|note| values(note).iter().any(|value| value == selected)),
        );
    }

    let mut available = BTreeSet::new();
    for note in index.active_notes() {
        available.extend(values(note).iter().map(String::as_str));
    }
    for value in available {
        println!("{value}");
    }
    Ok(())
}

fn print_note_list<'a>(notes: impl IntoIterator<Item = &'a NoteMeta>) -> Result<()> {
    let color = crate::terminal::stdout_color_enabled();
    for note in notes {
        println!("{}", summary_line_for_display(note, color));
    }
    Ok(())
}

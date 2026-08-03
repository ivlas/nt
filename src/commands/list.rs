use std::collections::BTreeSet;
use std::io::{self, IsTerminal};

use crate::display::summary_line_for_display;
use crate::error::Result;
use crate::listing::{ListRequest, render_link_row, render_link_table, render_row, render_table};
use crate::query::Query;
use crate::repository::{NoteMeta, Repository};

use super::{validate_collection, validate_source, validate_tag};

pub(super) fn list(args: &[String]) -> Result<()> {
    let repository = Repository::open()?;
    match ListRequest::parse(args)? {
        ListRequest::Notes { fields, query } => {
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
        ListRequest::Tags(tag) => {
            let notes = repository.list_notes()?;
            list_metadata(&notes, tag.as_deref(), validate_tag, |note| &note.tags)
        }
        ListRequest::Collections(collection) => {
            if let Some(collection) = collection.as_deref() {
                let notes = repository.list_notes()?;
                list_metadata(&notes, Some(collection), validate_collection, |note| {
                    &note.collections
                })
            } else {
                for collection in repository.list_collections()? {
                    println!("{collection}");
                }
                Ok(())
            }
        }
        ListRequest::Sources(source) => {
            let notes = repository.list_notes()?;
            list_metadata(&notes, source.as_deref(), validate_source, |note| {
                &note.sources
            })
        }
        ListRequest::LinkGraph { query, from, to } => {
            let notes = repository.list_notes()?;
            list_link_graph(&notes, &query, from.as_deref(), to.as_deref())
        }
    }
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

fn list_link_graph(
    notes: &[NoteMeta],
    query: &Query,
    from_id: Option<&str>,
    to_id: Option<&str>,
) -> Result<()> {
    let links = matching_notes(notes, query)?
        .into_iter()
        .filter(|note| from_id.is_none_or(|id| note.id == id))
        .flat_map(move |from| {
            from.links
                .iter()
                .filter(move |id| to_id.is_none_or(|selected| id.as_str() == selected))
                .filter_map(move |id| {
                    notes
                        .iter()
                        .find(|candidate| candidate.id == *id)
                        .map(|to| (from, to))
                })
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
    notes: &'a [NoteMeta],
    selected: Option<&str>,
    validate: impl Fn(&str) -> Result<()>,
    values: impl Fn(&'a NoteMeta) -> &'a [String],
) -> Result<()> {
    if let Some(selected) = selected {
        validate(selected)?;
        return print_note_list(
            notes
                .iter()
                .filter(|note| values(note).iter().any(|value| value == selected)),
        );
    }

    let mut available = BTreeSet::new();
    for note in notes {
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

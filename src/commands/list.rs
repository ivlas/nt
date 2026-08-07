use std::io::{self, IsTerminal};

use crate::error::Result;
use crate::query::NoteQuery;
use crate::repository::{NoteSummary, Repository};

const NOTE_HEADERS: [&str; 5] = ["id", "updated", "collection", "title", "tags"];

pub(super) fn list(arguments: &[String]) -> Result<()> {
    match arguments {
        [target] if target == "tags" => {
            let repository = Repository::open()?;
            print_values("tag", repository.list_tags()?)
        }
        [target] if target == "collections" => {
            let repository = Repository::open()?;
            print_values("collection", repository.list_collections()?)
        }
        filters => {
            let query = NoteQuery::parse_list(filters)?;
            let repository = Repository::open()?;
            print_notes(repository.list_notes(&query)?)
        }
    }
}

pub(super) fn print_notes(notes: Vec<NoteSummary>) -> Result<()> {
    if io::stdout().is_terminal() {
        let rows = notes.iter().map(note_row).collect::<Vec<_>>();
        print!("{}", format_table(NOTE_HEADERS, &rows));
    } else {
        for note in notes {
            print_redirected(&note)?;
        }
    }
    Ok(())
}

fn note_row(note: &NoteSummary) -> [String; 5] {
    let tags = note
        .tags()
        .iter()
        .map(|tag| tag.as_str())
        .collect::<Vec<_>>()
        .join(",");
    [
        note.id().to_string(),
        note.updated().to_string(),
        note.collection().to_string(),
        note.title().to_string(),
        tags,
    ]
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

fn print_values<T: AsRef<str>>(header: &str, values: Vec<T>) -> Result<()> {
    if io::stdout().is_terminal() {
        let rows = values
            .iter()
            .map(|value| [value.as_ref().to_string()])
            .collect::<Vec<_>>();
        print!("{}", format_table([header], &rows));
    } else {
        for value in values {
            println!("{}", serde_json::to_string(value.as_ref())?);
        }
    }
    Ok(())
}

fn format_table<const N: usize>(headers: [&str; N], rows: &[[String; N]]) -> String {
    let widths = std::array::from_fn(|column| {
        rows.iter()
            .map(|row| row[column].chars().count())
            .chain([headers[column].chars().count()])
            .max()
            .unwrap_or_default()
    });
    let mut output = String::new();
    append_row(&mut output, headers, &widths);
    for row in rows {
        append_row(&mut output, row.each_ref().map(String::as_str), &widths);
    }
    output
}

fn append_row<const N: usize>(output: &mut String, cells: [&str; N], widths: &[usize; N]) {
    for (column, cell) in cells.iter().enumerate() {
        output.push_str(cell);
        if column + 1 < N {
            let padding = widths[column] - cell.chars().count() + 2;
            output.extend(std::iter::repeat_n(' ', padding));
        }
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::format_table;

    #[test]
    fn tty_tables_include_headers_and_align_columns() {
        let rows = [
            ["1".to_string(), "inbox".to_string(), "Short".to_string()],
            [
                "22".to_string(),
                "work/project".to_string(),
                "Longer".to_string(),
            ],
        ];

        assert_eq!(
            format_table(["id", "collection", "title"], &rows),
            "id  collection    title\n1   inbox         Short\n22  work/project  Longer\n"
        );
    }

    #[test]
    fn empty_tty_tables_still_include_headers() {
        assert_eq!(format_table(["tag"], &[]), "tag\n");
    }
}

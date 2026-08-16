use std::io::{self, BufRead, BufReader, BufWriter, Seek, Write};

use crate::domains::note::{NoteQuery, NoteSummary, Repository};
use crate::error::Result;

const NOTE_HEADERS: [&str; 6] = ["id", "updated", "collection", "title", "tags", "outgoing"];

pub(crate) fn print_notes(
    repository: &Repository,
    query: &NoteQuery,
    output: &mut dyn Write,
    output_is_terminal: bool,
) -> Result<()> {
    if output_is_terminal {
        let mut output = BufWriter::new(output);
        write_spooled_table(&mut output, NOTE_HEADERS, |write_row| {
            repository.visit_note_summaries(query, |note| write_row(note_row(&note)))
        })?;
        output.flush()?;
    } else {
        let mut output = BufWriter::new(output);
        match repository.visit_note_summaries(query, |note| print_redirected(&mut output, &note)) {
            Err(crate::error::NtError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => {
                return Ok(());
            }
            result => result?,
        }
        if let Err(error) = output.flush() {
            if error.kind() == io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(error.into());
        }
    }
    Ok(())
}

fn note_row(note: &NoteSummary) -> [String; 6] {
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
        note.outgoing().to_string(),
    ]
}

fn print_redirected(output: &mut impl Write, note: &NoteSummary) -> Result<()> {
    let tags = note
        .tags()
        .iter()
        .map(|tag| tag.as_str())
        .collect::<Vec<_>>();
    writeln!(
        output,
        "{}\t{}\t{}\t{}\t{}\t{}",
        serde_json::to_string(&note.id().to_string())?,
        serde_json::to_string(note.updated().as_str())?,
        serde_json::to_string(note.collection().as_str())?,
        serde_json::to_string(note.title())?,
        serde_json::to_string(&tags)?,
        serde_json::to_string(&note.outgoing())?,
    )?;
    Ok(())
}

fn write_spooled_table<const N: usize>(
    output: &mut impl Write,
    headers: [&str; N],
    produce: impl FnOnce(&mut dyn FnMut([String; N]) -> Result<()>) -> Result<()>,
) -> Result<()> {
    let mut spool = tempfile::tempfile()?;
    let mut widths = headers.map(|header| header.chars().count());
    {
        let mut spool_output = BufWriter::new(&mut spool);
        {
            let mut write_row = |row: [String; N]| {
                for (column, cell) in row.iter().enumerate() {
                    widths[column] = widths[column].max(cell.chars().count());
                }
                writeln!(spool_output, "{}", serde_json::to_string(row.as_slice())?)?;
                Ok(())
            };
            produce(&mut write_row)?;
        }
        spool_output.flush()?;
    }
    spool.rewind()?;

    write_table_row(output, headers, &widths)?;
    for line in BufReader::new(spool).lines() {
        let row: [String; N] = serde_json::from_str::<Vec<String>>(&line?)?
            .try_into()
            .expect("spooled table rows preserve their column count");
        write_table_row(output, row.each_ref().map(String::as_str), &widths)?;
    }
    Ok(())
}

fn write_table_row<const N: usize>(
    output: &mut impl Write,
    cells: [&str; N],
    widths: &[usize; N],
) -> io::Result<()> {
    const SPACES: [u8; 64] = [b' '; 64];

    for (column, cell) in cells.iter().enumerate() {
        output.write_all(cell.as_bytes())?;
        if column + 1 < N {
            let mut padding = widths[column] - cell.chars().count() + 2;
            while padding > 0 {
                let chunk = padding.min(SPACES.len());
                output.write_all(&SPACES[..chunk])?;
                padding -= chunk;
            }
        }
    }
    output.write_all(b"\n")
}

pub(crate) fn print_values<T: AsRef<str>>(
    output: &mut dyn Write,
    output_is_terminal: bool,
    header: &str,
    values: Vec<T>,
) -> Result<()> {
    if output_is_terminal {
        let rows = values
            .iter()
            .map(|value| [value.as_ref().to_string()])
            .collect::<Vec<_>>();
        output.write_all(format_table([header], &rows).as_bytes())?;
    } else {
        for value in values {
            writeln!(output, "{}", serde_json::to_string(value.as_ref())?)?;
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
    use std::io::{self, Write};

    use super::{format_table, write_spooled_table};

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

    #[test]
    fn spooled_tables_preserve_tty_alignment() {
        let rows = [
            ["1".to_string(), "inbox".to_string(), "Café".to_string()],
            [
                "22".to_string(),
                "work/project".to_string(),
                "Longer".to_string(),
            ],
        ];
        let mut output = Vec::new();

        write_spooled_table(&mut output, ["id", "collection", "title"], |write_row| {
            for row in &rows {
                write_row(row.clone())?;
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            format_table(["id", "collection", "title"], &rows)
        );
    }

    #[test]
    fn spooled_tables_handle_large_streams_without_collecting_output() {
        let mut output = CountingWriter::default();

        write_spooled_table(&mut output, ["value"], |write_row| {
            for index in 0..10_000 {
                write_row([format!("row {index}")])?;
            }
            Ok(())
        })
        .unwrap();

        assert!(output.bytes > 80_000);
    }

    #[derive(Default)]
    struct CountingWriter {
        bytes: usize,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.bytes += buffer.len();
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

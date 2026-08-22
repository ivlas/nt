use std::io::{self, BufRead, BufReader, BufWriter, Seek, Write};

use unicode_width::UnicodeWidthStr;

use super::output::write_stream;
use crate::error::{NtError, Result, StoredNoteContext};
use crate::note::{NoteQuery, NoteSummary, Repository};

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
        write_stream(output, |output| {
            repository.visit_note_summaries(query, |note| print_redirected(output, &note))
        })?;
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
        escape_terminal_controls(note.title()),
        tags,
        note.outgoing().to_string(),
    ]
}

fn escape_terminal_controls(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

fn print_redirected(output: &mut (impl Write + ?Sized), note: &NoteSummary) -> Result<()> {
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
    let mut widths = headers.map(display_width);
    {
        let mut spool_output = BufWriter::new(&mut spool);
        {
            let mut write_row = |row: [String; N]| {
                for (column, cell) in row.iter().enumerate() {
                    widths[column] = widths[column].max(display_width(cell));
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
        let row = decode_spooled_row(&line?)?;
        write_table_row(output, row.each_ref().map(String::as_str), &widths)?;
    }
    Ok(())
}

fn decode_spooled_row<const N: usize>(line: &str) -> Result<[String; N]> {
    serde_json::from_str::<Vec<String>>(line)?
        .try_into()
        .map_err(|_| NtError::invalid_stored(StoredNoteContext::new(None, None), "spooled row"))
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
            let mut padding = widths[column] - display_width(cell) + 2;
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
        write_stream(output, |output| {
            for value in values {
                let encoded = serde_json::to_string(value.as_ref())?;
                writeln!(output, "{encoded}")?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn format_table<const N: usize>(headers: [&str; N], rows: &[[String; N]]) -> String {
    let widths = std::array::from_fn(|column| {
        rows.iter()
            .map(|row| display_width(&row[column]))
            .chain([display_width(headers[column])])
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
            let padding = widths[column] - display_width(cell) + 2;
            output.extend(std::iter::repeat_n(' ', padding));
        }
    }
    output.push('\n');
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{
        decode_spooled_row, display_width, escape_terminal_controls, format_table, print_values,
        write_spooled_table,
    };
    use crate::error::NtError;

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
    fn tty_tables_use_unicode_display_width() {
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
        assert_eq!(display_width("🙂"), 2);

        let rows = [
            ["界".to_string(), "wide".to_string()],
            ["e\u{301}".to_string(), "combining".to_string()],
            ["🙂".to_string(), "emoji".to_string()],
        ];
        assert_eq!(
            format_table(["v", "kind"], &rows),
            "v   kind\n界  wide\ne\u{301}   combining\n🙂  emoji\n"
        );
    }

    #[test]
    fn tty_titles_escape_controls_without_escaping_unicode() {
        let escaped = escape_terminal_controls("A\tB\u{1b}]界");

        assert_eq!(escaped, r"A\tB\u{1b}]界");
        assert!(!escaped.chars().any(char::is_control));
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

    #[test]
    fn malformed_spooled_column_counts_return_stored_data_errors() {
        let error = decode_spooled_row::<2>(r#"["only one"]"#).unwrap_err();

        assert!(matches!(
            error,
            NtError::InvalidStoredNote {
                field: "spooled row",
                ..
            }
        ));
    }

    #[test]
    fn redirected_inventories_ignore_broken_pipes() {
        print_values(
            &mut BrokenPipeWriter,
            false,
            "tag",
            vec!["rust".to_string()],
        )
        .unwrap();
    }

    #[derive(Default)]
    struct CountingWriter {
        bytes: usize,
    }

    struct BrokenPipeWriter;

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.bytes += buffer.len();
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

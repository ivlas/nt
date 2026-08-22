use std::io::{self, BufWriter, Write};

use crate::cli::MemoryCommand;
use crate::error::{NtError, Result};
use crate::memory::{
    ContextItem, ExpansionItem, MEMORY_CONTEXT_CHARS, Memory, MemoryContextQuery, MemoryListQuery,
    MemoryRecallQuery, NewMemory, NewSummary, RawRange, Repository, SummaryNodeId, range,
};
use crate::schema;

use super::{App, write_commit_output};

pub(super) fn memory(app: &mut App<'_>, command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Add { body } => add(app, &body),
        MemoryCommand::Show { seq } => show(app, &seq),
        MemoryCommand::List { filters } => list(app, &filters),
        MemoryCommand::Recall { expressions } => recall(app, &expressions),
        MemoryCommand::Context { terms } => context(app, &terms),
        MemoryCommand::Pending { arguments } => pending(app, &arguments),
        MemoryCommand::Summarize { node, summary } => summarize(app, &node, &summary),
        MemoryCommand::Expand { node } => expand(app, &node),
        MemoryCommand::Invalidate { node } => invalidate(app, &node),
        MemoryCommand::Status => status(app),
    }
}

fn add(app: &mut App<'_>, body_arguments: &[String]) -> Result<()> {
    let body = app.input.read_memory_body(body_arguments)?;
    let memory = NewMemory::new(body)?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    let seq = repository.append(memory)?;
    write_commit_output(app.output, format_args!("saved {seq}\n"))
}

fn show(app: &mut App<'_>, seq: &str) -> Result<()> {
    let seq = parse_positive(seq, "memory seq")?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    let memory = repository.get_memory(seq)?;
    app.output.write_all(memory.body().as_bytes())?;
    app.output.flush()?;
    Ok(())
}

fn list(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = MemoryListQuery::parse(expressions)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    write_memories(app.output, |write| {
        repository.visit_memories(&query, |memory| write(&memory))
    })
}

fn recall(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = MemoryRecallQuery::parse(expressions)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    write_memories(app.output, |write| {
        repository.visit_recalled(&query, |memory| write(&memory))
    })
}

fn context(app: &mut App<'_>, expressions: &[String]) -> Result<()> {
    let query = MemoryContextQuery::parse(expressions)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    let items = repository.context(&query)?;
    let document = render_context(&items)?;
    app.output.write_all(document.as_bytes())?;
    Ok(())
}

fn render_context(items: &[ContextItem]) -> Result<String> {
    let mut document = String::new();
    for (index, item) in items.iter().enumerate() {
        if index != 0 {
            document.push('\n');
        }
        document.push_str(&item.context_header()?);
        document.push_str(item.content());
        document.push('\n');
    }
    if document.chars().count() > MEMORY_CONTEXT_CHARS {
        return Err(NtError::MemoryContextOverflow);
    }
    Ok(document)
}

fn pending(app: &mut App<'_>, arguments: &[String]) -> Result<()> {
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    match arguments {
        [] => write_pending_jobs(app.output, |write| {
            repository.visit_pending(None, |job| write(&job))
        }),
        [argument] if argument.starts_with("limit:") => {
            let limit = parse_positive(
                argument.strip_prefix("limit:").unwrap_or_default(),
                "memory pending limit",
            )?;
            write_pending_jobs(app.output, |write| {
                repository.visit_pending(Some(limit), |job| write(&job))
            })
        }
        [node] => {
            let node: SummaryNodeId = node.parse()?;
            let children = repository.inspect_pending(node)?;
            write_summary_task(app.output, node, &children)
        }
        _ => Err(NtError::InvalidValue {
            field: "memory pending arguments",
            value: arguments.join(" "),
        }),
    }
}

fn summarize(app: &mut App<'_>, node: &str, summary_arguments: &[String]) -> Result<()> {
    let node: SummaryNodeId = node.parse()?;
    let summary = app.input.read_memory_body(summary_arguments)?;
    let summary = NewSummary::new(summary)?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.summarize(node, summary)?;
    write_commit_output(app.output, format_args!("summarized {node}\n"))
}

fn expand(app: &mut App<'_>, node: &str) -> Result<()> {
    let node: SummaryNodeId = node.parse()?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    let children = repository.expand(node)?;
    write_expansion(app.output, &children)
}

fn invalidate(app: &mut App<'_>, node: &str) -> Result<()> {
    let node: SummaryNodeId = node.parse()?;
    let mut repository =
        Repository::from_connection(schema::open_read_write(app.database_path()?)?);
    repository.invalidate(node)?;
    write_commit_output(app.output, format_args!("invalidated {node}\n"))
}

fn status(app: &mut App<'_>) -> Result<()> {
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    let status = repository.status()?;
    writeln!(app.output, "raw memory count\t{}", status.raw_count())?;
    writeln!(
        app.output,
        "highest sequence\t{}",
        status
            .highest_seq()
            .map_or_else(|| "none".to_string(), |value| value.to_string())
    )?;
    writeln!(app.output, "summary count\t{}", status.summary_count())?;
    writeln!(
        app.output,
        "pending summary count\t{}",
        status.pending_count()
    )?;
    writeln!(
        app.output,
        "highest completed level\t{}",
        status
            .highest_completed_level()
            .map_or_else(|| "none".to_string(), |value| value.to_string())
    )?;
    Ok(())
}

fn write_memories(
    output: &mut dyn Write,
    produce: impl FnOnce(&mut dyn FnMut(&Memory) -> Result<()>) -> Result<()>,
) -> Result<()> {
    let mut output = BufWriter::new(output);
    let mut write = |memory: &Memory| write_memory_row(&mut output, memory);
    match produce(&mut write) {
        Err(NtError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
        result => result?,
    }
    match output.flush() {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
        Ok(()) => Ok(()),
    }
}

fn write_memory_row(output: &mut (impl Write + ?Sized), memory: &Memory) -> Result<()> {
    writeln!(
        output,
        "{}\t{}\t{}",
        memory.seq(),
        serde_json::to_string(memory.created().as_str())?,
        serde_json::to_string(memory.body())?
    )?;
    Ok(())
}

fn write_pending_jobs(
    output: &mut dyn Write,
    produce: impl FnOnce(&mut dyn FnMut(&crate::memory::PendingJob) -> Result<()>) -> Result<()>,
) -> Result<()> {
    let mut output = BufWriter::new(output);
    let mut write = |job: &crate::memory::PendingJob| {
        writeln!(
            output,
            "{}\t{}-{}\t{}",
            job.node(),
            job.raw_range().start(),
            job.raw_range().end(),
            job.node().level()
        )?;
        Ok(())
    };
    match produce(&mut write) {
        Err(NtError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
        result => result?,
    }
    match output.flush() {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
        Ok(()) => Ok(()),
    }
}

fn write_summary_task(
    output: &mut dyn Write,
    node: SummaryNodeId,
    children: &[ExpansionItem],
) -> Result<()> {
    let raw = rendered_range(node)?;
    writeln!(output, "node\t{node}")?;
    writeln!(output, "raw range\t{}-{}", raw.start(), raw.end())?;
    writeln!(output, "level\t{}", node.level())?;
    writeln!(output)?;
    write_expansion(output, children)?;
    output.write_all(
        b"\nCompress these children into one factual summary.\n\
Keep durable information.\n\
Preserve decisions, outcomes, constraints and important changes.\n\
Invent nothing.\n\
Maximum: 1024 characters.\n",
    )?;
    Ok(())
}

fn write_expansion(output: &mut dyn Write, children: &[ExpansionItem]) -> Result<()> {
    for child in children {
        match child {
            ExpansionItem::Raw(memory) => write_memory_row(output, memory)?,
            ExpansionItem::Summary(segment) => {
                let raw = rendered_range(segment.node())?;
                writeln!(
                    output,
                    "{}\t{}-{}\t{}\t{}",
                    segment.node(),
                    raw.start(),
                    raw.end(),
                    serde_json::to_string(segment.created().as_str())?,
                    serde_json::to_string(segment.summary())?
                )?;
            }
        }
    }
    Ok(())
}

fn rendered_range(node: SummaryNodeId) -> Result<RawRange> {
    range(node.level(), node.block()).ok_or_else(|| NtError::InvalidValue {
        field: "memory node",
        value: format!("{node} has an invalid raw range"),
    })
}

fn parse_positive(value: &str, field: &'static str) -> Result<i64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return invalid_positive(field, value);
    }
    let parsed = value.parse::<i64>().map_err(|_| NtError::InvalidValue {
        field,
        value: value.to_string(),
    })?;
    if parsed == 0 {
        return invalid_positive(field, value);
    }
    Ok(parsed)
}

fn invalid_positive<T>(field: &'static str, value: &str) -> Result<T> {
    Err(NtError::InvalidValue {
        field,
        value: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{render_context, write_expansion, write_memories, write_pending_jobs};
    use crate::error::NtError;
    use crate::memory::schema::OBJECTS;
    use crate::memory::{
        ExpansionItem, MEMORY_CONTEXT_CHARS, Memory, MemoryContextQuery, MemorySegment, NewMemory,
        Repository, SummaryNodeId,
    };

    fn memory() -> Memory {
        Memory::from_new(
            1,
            NewMemory::new("body").unwrap(),
            "2026-08-22T12:34:56Z".parse().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn streamed_memory_output_treats_broken_writes_and_flushes_as_success() {
        write_memories(&mut BrokenPipeWriter, |write| write(&memory())).unwrap();
        write_memories(&mut FlushBrokenPipeWriter, |write| write(&memory())).unwrap();
    }

    #[test]
    fn streamed_pending_output_stops_before_decoding_later_malformed_rows() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        for object in OBJECTS {
            connection.execute_batch(object.sql).unwrap();
        }
        connection
            .execute_batch(
                "WITH RECURSIVE jobs(block) AS (
                     VALUES(0)
                     UNION ALL
                     SELECT block + 1 FROM jobs WHERE block < 1999
                 )
                 INSERT INTO memory_summary_jobs(level, block)
                 SELECT 0, block FROM jobs;
                 PRAGMA ignore_check_constraints = ON;
                 INSERT INTO memory_summary_jobs(level, block) VALUES (14, 7);
                 PRAGMA ignore_check_constraints = OFF;",
            )
            .unwrap();
        let repository = Repository::from_connection(connection);

        write_pending_jobs(&mut BrokenPipeWriter, |write| {
            repository.visit_pending(None, |job| write(&job))
        })
        .unwrap();
    }

    #[test]
    fn expansion_rendering_rejects_invalid_stored_summary_ranges() {
        let segment = MemorySegment::from_stored(
            1,
            SummaryNodeId::new(14, 7).unwrap(),
            "summary".to_string(),
            "2026-08-22T12:34:56Z".parse().unwrap(),
        )
        .unwrap();
        let error =
            write_expansion(&mut Vec::new(), &[ExpansionItem::Summary(segment)]).unwrap_err();

        assert!(matches!(
            error,
            NtError::InvalidValue {
                field: "memory node",
                ..
            }
        ));
    }

    #[test]
    fn rendered_context_including_headers_never_exceeds_the_limit() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        for object in OBJECTS {
            connection.execute_batch(object.sql).unwrap();
        }
        let mut repository = Repository::from_connection(connection);
        let body = format!("needle{}", "é".repeat(1_018));
        for _ in 0..40 {
            repository.append(NewMemory::new(&body).unwrap()).unwrap();
        }
        let query = MemoryContextQuery::parse(&["needle".to_string()]).unwrap();
        let document = render_context(&repository.context(&query).unwrap()).unwrap();

        assert!(document.chars().count() <= MEMORY_CONTEXT_CHARS);
        assert!(document.contains("# memory "));
        assert!(document.contains("2026-"));
        assert!(document.contains(&body));
    }

    struct BrokenPipeWriter;
    struct FlushBrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for FlushBrokenPipeWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }
    }
}

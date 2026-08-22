use std::io::Write;

use crate::cli::output::write_stream;
use crate::error::{NtError, Result};
use crate::memory::{
    ContextItem, ExpansionItem, MEMORY_CONTEXT_CHARS, Memory, MemoryStatus, PendingJob, RawRange,
    SummaryNodeId, range,
};

pub(super) fn render_context(items: &[ContextItem]) -> Result<String> {
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

pub(super) fn write_memories(
    output: &mut dyn Write,
    produce: impl FnOnce(&mut dyn FnMut(&Memory) -> Result<()>) -> Result<()>,
) -> Result<()> {
    write_stream(output, |output| {
        let mut write = |memory: &Memory| write_memory_row(output, memory);
        produce(&mut write)
    })
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

pub(super) fn write_pending_jobs(
    output: &mut dyn Write,
    produce: impl FnOnce(&mut dyn FnMut(&PendingJob) -> Result<()>) -> Result<()>,
) -> Result<()> {
    write_stream(output, |output| {
        let mut write = |job: &PendingJob| {
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
        produce(&mut write)
    })
}

pub(super) fn write_summary_task(
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

pub(super) fn write_expansion(output: &mut dyn Write, children: &[ExpansionItem]) -> Result<()> {
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

pub(super) fn write_status(output: &mut dyn Write, status: &MemoryStatus) -> Result<()> {
    writeln!(output, "raw memory count\t{}", status.raw_count())?;
    writeln!(
        output,
        "highest sequence\t{}",
        status
            .highest_seq()
            .map_or_else(|| "none".to_string(), |value| value.to_string())
    )?;
    writeln!(output, "summary count\t{}", status.summary_count())?;
    writeln!(output, "pending summary count\t{}", status.pending_count())?;
    writeln!(
        output,
        "highest completed level\t{}",
        status
            .highest_completed_level()
            .map_or_else(|| "none".to_string(), |value| value.to_string())
    )?;
    Ok(())
}

fn rendered_range(node: SummaryNodeId) -> Result<RawRange> {
    range(node.level(), node.block()).ok_or_else(|| NtError::InvalidValue {
        field: "memory node",
        value: format!("{node} has an invalid raw range"),
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
    fn streamed_memory_output_treats_broken_writes_as_success() {
        write_memories(&mut BrokenPipeWriter, |write| write(&memory())).unwrap();
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

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

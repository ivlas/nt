use std::io::Write;

use crate::cli::MemoryCommand;
use crate::cli::output::write_stream;
use crate::error::{NtError, Result};
use crate::memory::{
    MemoryRange, NewMemory, NewSummary, Repository, TreeItem, WAKE_ENTRIES, wake_cover,
};
use crate::schema;

use super::{App, write_commit_output};

pub(super) fn memory(app: &mut App<'_>, command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Add { body } => add(app, &body),
        MemoryCommand::Wake => wake(app),
        MemoryCommand::Recall { pattern } => recall(app, &pattern),
        MemoryCommand::Nap { range, summary } => nap(app, range.as_deref(), &summary),
        MemoryCommand::Zoom { range } => zoom(app, &range),
        MemoryCommand::Forget { range } => forget(app, &range),
    }
}

fn add(app: &mut App<'_>, arguments: &[String]) -> Result<()> {
    let memory = NewMemory::new(app.input.read_memory_body(arguments)?)?;
    let mut repository = write_repository(app)?;
    let sequence = repository.append_memory(memory)?;
    write_commit_output(app.output, format_args!("saved #{sequence}\n"))
}

fn wake(app: &mut App<'_>) -> Result<()> {
    let repository = read_repository(app)?;
    let cover = wake_cover(repository.raw_count()?, WAKE_ENTRIES)?;
    write_items(app.output, &repository.wake_items(&cover)?)
}

fn recall(app: &mut App<'_>, arguments: &[String]) -> Result<()> {
    let pattern = arguments.join(" ");
    if pattern.is_empty() {
        return Err(NtError::InvalidValue {
            field: "recall pattern",
            value: "empty".to_string(),
        });
    }
    let repository = read_repository(app)?;
    write_stream(app.output, |output| {
        repository.recall(&pattern, |memory| {
            writeln!(output, "#{} {}", memory.seq(), memory.body())?;
            Ok(())
        })
    })
}

fn nap(app: &mut App<'_>, range: Option<&str>, arguments: &[String]) -> Result<()> {
    let Some(range) = range else {
        if !arguments.is_empty() {
            return Err(NtError::InvalidValue {
                field: "memory nap",
                value: "summary requires a range".to_string(),
            });
        }
        let repository = read_repository(app)?;
        let Some(range) = repository.next_summary()? else {
            app.output.write_all(b"nothing to nap\n")?;
            return Ok(());
        };
        let inputs = repository.summary_inputs(range)?;
        writeln!(
            app.output,
            "Compress memories #{range} into one short memory:\n"
        )?;
        write_items(app.output, &inputs)?;
        writeln!(app.output, "\nRun:\nnt memory nap {range} \"<summary>\"")?;
        return Ok(());
    };

    let range: MemoryRange = range.parse()?;
    let summary = NewSummary::new(app.input.read_memory_body(arguments)?)?;
    let mut repository = write_repository(app)?;
    repository.put_summary(range, summary)?;
    write_commit_output(app.output, format_args!("summarized #{range}\n"))
}

fn zoom(app: &mut App<'_>, range: &str) -> Result<()> {
    let range: MemoryRange = range.parse()?;
    let repository = read_repository(app)?;
    write_items(app.output, &repository.zoom(range)?)
}

fn forget(app: &mut App<'_>, range: &str) -> Result<()> {
    let range: MemoryRange = range.parse()?;
    let mut repository = write_repository(app)?;
    repository.forget(range)?;
    write_commit_output(app.output, format_args!("forgot #{range}\n"))
}

fn write_items(output: &mut dyn Write, items: &[TreeItem]) -> Result<()> {
    write_stream(output, |output| {
        for item in items {
            match item {
                TreeItem::Raw(memory) => writeln!(output, "#{} {}", memory.seq(), memory.body())?,
                TreeItem::Summary(summary) => {
                    writeln!(output, "#{} {}", summary.range(), summary.body())?
                }
            }
        }
        Ok(())
    })
}

fn read_repository(app: &App<'_>) -> Result<Repository> {
    schema::open_read_only(app.database_path()?).map(Repository::from_connection)
}

fn write_repository(app: &App<'_>) -> Result<Repository> {
    schema::open_read_write(app.database_path()?).map(Repository::from_connection)
}

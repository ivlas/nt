use crate::cli::MemoryCommand;
use crate::error::Result;
use crate::memory::{
    MemoryContextQuery, MemoryListQuery, MemoryRecallQuery, NewMemory, NewSummary, Repository,
    SummaryNodeId,
};
use crate::schema;

use super::{App, write_commit_output};
use arguments::{PendingRequest, parse_pending, parse_positive};
use output::{
    render_context, write_expansion, write_memories, write_pending_jobs, write_status,
    write_summary_task,
};

mod arguments;
mod output;

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

fn pending(app: &mut App<'_>, arguments: &[String]) -> Result<()> {
    let request = parse_pending(arguments)?;
    let repository = Repository::from_connection(schema::open_read_only(app.database_path()?)?);
    match request {
        PendingRequest::List(limit) => write_pending_jobs(app.output, |write| {
            repository.visit_pending(limit, |job| write(&job))
        }),
        PendingRequest::Inspect(node) => {
            let children = repository.inspect_pending(node)?;
            write_summary_task(app.output, node, &children)
        }
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
    write_status(app.output, &repository.status()?)
}

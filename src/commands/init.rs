use crate::core::storage::InitOutcome;
use crate::domains::note::Repository;
use crate::error::Result;

use super::{App, write_commit_output};

pub(super) fn init(app: &mut App<'_>) -> Result<()> {
    let message = match Repository::initialize_at(app.database_path()?)? {
        InitOutcome::Initialized => "initialized",
        InitOutcome::AlreadyInitialized => "already initialized",
    };
    write_commit_output(app.output, format_args!("{message}\n"))?;
    Ok(())
}

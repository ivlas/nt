use crate::error::Result;
use crate::note::Repository;
use crate::storage::InitOutcome;

use super::{App, write_commit_output};

pub(super) fn init(app: &mut App<'_>) -> Result<()> {
    let message = match Repository::initialize_at(app.database_path()?)? {
        InitOutcome::Initialized => "initialized",
        InitOutcome::AlreadyInitialized => "already initialized",
    };
    write_commit_output(app.output, format_args!("{message}\n"))?;
    Ok(())
}

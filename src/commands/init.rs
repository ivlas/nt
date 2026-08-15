use crate::error::Result;
use crate::repository::{InitOutcome, Repository};

use super::{App, write_commit_output};

pub(super) fn init(app: &mut App<'_>) -> Result<()> {
    let message = match Repository::initialize_at(app.database_path()?)? {
        InitOutcome::Initialized => "initialized",
        InitOutcome::AlreadyInitialized => "already initialized",
    };
    write_commit_output(app.output, format_args!("{message}\n"))?;
    Ok(())
}

use crate::error::Result;
use crate::repository::{InitOutcome, Repository};

use super::App;

pub(super) fn init(app: &mut App<'_>) -> Result<()> {
    match Repository::initialize_at(app.database_path()?)? {
        InitOutcome::Initialized => writeln!(app.output, "initialized")?,
        InitOutcome::AlreadyInitialized => writeln!(app.output, "already initialized")?,
    }
    Ok(())
}

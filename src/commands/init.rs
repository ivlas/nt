use crate::error::Result;
use crate::repository::{InitOutcome, Repository};

pub(super) fn init() -> Result<()> {
    match Repository::initialize()? {
        InitOutcome::Initialized => println!("initialized"),
        InitOutcome::AlreadyInitialized => println!("already initialized"),
    }
    Ok(())
}

use crate::error::Result;
use crate::repository::Repository;

pub(super) fn init(vault: &str) -> Result<()> {
    let repository = Repository::open_for_init()?;
    let now = crate::note::timestamp_now().iso;
    repository.create_vault(vault, &now)?;
    println!("initialized {vault}");
    Ok(())
}

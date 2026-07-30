use crate::error::Result;
use crate::index::Index;

pub(super) fn init(vault: &str) -> Result<()> {
    let mut index = Index::load()?;
    let now = crate::note::timestamp_now().iso;
    index.create_vault(vault, &now)?;
    index.save()?;
    println!("initialized {vault}");
    Ok(())
}

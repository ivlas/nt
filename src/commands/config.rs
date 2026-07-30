use crate::cli::ConfigCommand;
use crate::error::Result;
use crate::fs::{database_path, relative_to_cwd};
use crate::index::Index;

pub(super) fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => {
            println!("database {}", relative_to_cwd(&database_path()?).display());
            Ok(())
        }
        ConfigCommand::Vault => {
            let index = Index::load()?;
            for vault in index.vaults.values() {
                println!("{}\t{}", vault.id, vault.name);
            }
            Ok(())
        }
    }
}

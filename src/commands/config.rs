use crate::cli::ConfigCommand;
use crate::error::Result;
use crate::fs::{database_path, relative_to_cwd};
use crate::repository::Repository;

pub(super) fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => {
            println!("database {}", relative_to_cwd(&database_path()?).display());
            Ok(())
        }
        ConfigCommand::Vault => {
            let repository = Repository::open()?;
            for vault in repository.list_vaults()? {
                println!("{}\t{}", vault.id, vault.name);
            }
            Ok(())
        }
    }
}

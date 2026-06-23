use crate::cli::ConfigCommand;
use crate::error::{NtError, Result};
use crate::fs::{IndexMutationLock, relative_to_cwd};
use crate::index::Index;

pub(super) fn config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show => config_show(),
        ConfigCommand::Vault { name } => match name {
            Some(name) => config_set_vault(&name),
            None => config_list_vaults(),
        },
    }
}

fn config_show() -> Result<()> {
    let index = Index::load()?;
    let vault = index.active_vault.as_deref().unwrap_or("-");
    let vault_path = index
        .active_vault_path()
        .map(relative_to_cwd)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());

    println!("vault {vault} {vault_path}");

    Ok(())
}

fn config_list_vaults() -> Result<()> {
    let index = Index::load()?;
    for (name, vault) in &index.vaults {
        let marker = if index.active_vault.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            "-"
        };
        println!("{marker} {name} {}", relative_to_cwd(&vault.path).display());
    }

    Ok(())
}

fn config_set_vault(name: &str) -> Result<()> {
    let _lock = IndexMutationLock::acquire()?;
    let mut index = Index::load()?;
    let Some(vault) = index.vaults.get(name) else {
        return Err(NtError::Message(format!(
            "unknown vault `{name}`; run `nt config vault`"
        )));
    };
    let path = vault.path.clone();

    index.active_vault = Some(name.to_string());
    index.save()?;

    println!(
        "configured vault {name} {}",
        relative_to_cwd(&path).display()
    );
    Ok(())
}

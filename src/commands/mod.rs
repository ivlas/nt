use crate::cli::{Cli, Command};
use crate::error::{NtError, Result};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[]),
        Some(Command::Help { topic }) => crate::cli::help::print(&topic),
        Some(command) => {
            consume(command);
            Err(NtError::Message(
                "command is not implemented in this build".to_string(),
            ))
        }
    }
}

fn consume(command: Command) {
    match command {
        Command::Init => {}
        Command::Add { metadata, body } => drop((metadata, body)),
        Command::Show { id } => drop(id),
        Command::List { filters } => drop(filters),
        Command::Find { expressions } => drop(expressions),
        Command::Rm { ids } => drop(ids),
        Command::Edit { id, body } => drop((id, body)),
        Command::Move { id, collection } => drop((id, collection)),
        Command::Tag { id, operation } | Command::Link { id, operation } => {
            drop((id, operation));
        }
        Command::Help { topic } => drop(topic),
    }
}

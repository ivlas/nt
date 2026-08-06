use crate::cli::{Cli, Command};
use crate::error::Result;

mod add;
mod init;
mod list;
mod rm;
mod show;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[]),
        Some(Command::Init) => init::init(),
        Some(Command::Add { metadata, body }) => add::add(&metadata, &body),
        Some(Command::Show { id }) => show::show(&id),
        Some(Command::List { filters }) => list::list(&filters),
        Some(Command::Rm { ids }) => rm::rm(&ids),
        Some(Command::Help { topic }) => crate::cli::help::print(&topic),
        Some(Command::Find { .. })
        | Some(Command::Edit { .. })
        | Some(Command::Move { .. })
        | Some(Command::Tag { .. })
        | Some(Command::Link { .. }) => Err(crate::error::NtError::Message(
            "command is not implemented in this build".to_string(),
        )),
    }
}

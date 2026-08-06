use crate::cli::{Cli, Command};
use crate::error::Result;

mod add;
mod edit;
mod init;
mod link;
mod list;
mod move_note;
mod rm;
mod show;
mod tag;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => crate::cli::help::print(&[]),
        Some(Command::Init) => init::init(),
        Some(Command::Add { metadata, body }) => add::add(&metadata, &body),
        Some(Command::Show { id }) => show::show(&id),
        Some(Command::List { filters }) => list::list(&filters),
        Some(Command::Rm { ids }) => rm::rm(&ids),
        Some(Command::Edit { id, body }) => edit::edit(&id, &body),
        Some(Command::Move { id, collection }) => move_note::move_note(&id, &collection),
        Some(Command::Tag { id, operation }) => tag::tag(&id, &operation),
        Some(Command::Link { id, operation }) => link::link(&id, &operation),
        Some(Command::Help { topic }) => crate::cli::help::print(&topic),
        Some(Command::Find { .. }) => Err(crate::error::NtError::Message(
            "command is not implemented in this build".to_string(),
        )),
    }
}

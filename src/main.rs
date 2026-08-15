use std::io::{self, IsTerminal};

use clap::Parser;

mod cli;
mod commands;
mod error;
mod fs;
mod input;
mod note;
mod query;
mod repository;
mod terminal;

fn main() {
    let cli = cli::Cli::parse();
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();
    let mut stdout = stdout.lock();
    let nt_home = fs::nt_home().ok();
    let database_path = nt_home.as_ref().map(|path| path.join("nt.sqlite3"));
    let visual = std::env::var("VISUAL").ok();
    let editor = std::env::var("EDITOR").ok();
    let mut editor_input = |seed: Option<String>| {
        input::read_editor(
            seed.as_deref(),
            nt_home.as_deref().ok_or(error::NtError::HomeNotFound)?,
            visual.as_deref(),
            editor.as_deref(),
        )
    };
    let input = input::Input::new(&mut stdin, stdin_is_terminal, &mut editor_input);
    let mut app = commands::App::new(database_path, input, &mut stdout, stdout_is_terminal);

    if let Err(error) = commands::run(cli, &mut app) {
        let message = format!("error: {error}");
        eprintln!(
            "{}",
            terminal::paint(
                &message,
                terminal::Style::Red,
                terminal::stderr_color_enabled()
            )
        );
        std::process::exit(1);
    }
}

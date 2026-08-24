//! `nt` is primarily a binary and CLI product.
//!
//! [`run_process`] is the only intentionally supported public Rust entry point.
//! Other crate modules and types are implementation details; there is currently
//! no stable embeddable SDK or library API. The internal `App` dependency
//! injection supports testing and command composition, not a public API promise.
//! Any public Rust API expansion should be deliberate rather than exposing
//! domain types incrementally.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use clap::Parser;

mod app;
mod cli;
mod commands;
mod error;
mod lexical;
mod note;
mod schema;
mod storage;

pub fn run_process() -> ExitCode {
    let cli = cli::Cli::parse();
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();
    let mut stdout = stdout.lock();
    let nt_home = cli::paths::nt_home().ok();
    let database_path = nt_home.as_ref().map(|path| path.join("nt.sqlite3"));
    let visual = std::env::var("VISUAL").ok();
    let editor = std::env::var("EDITOR").ok();
    let mut editor_input = |seed: Option<String>| {
        cli::input::read_editor(
            seed.as_deref(),
            nt_home.as_deref().ok_or(error::NtError::HomeNotFound)?,
            visual.as_deref(),
            editor.as_deref(),
        )
    };
    let input = cli::input::Input::new(&mut stdin, stdin_is_terminal, &mut editor_input);
    let mut app = app::App::new(database_path, input, &mut stdout, stdout_is_terminal);

    if let Err(error) = commands::run(cli, &mut app) {
        let exit_code = error.exit_code();
        let message = format!("error: {error}");
        eprintln!(
            "{}",
            cli::terminal::paint(
                &message,
                cli::terminal::Style::Red,
                cli::terminal::stderr_color_enabled()
            )
        );
        return ExitCode::from(exit_code);
    }

    ExitCode::SUCCESS
}

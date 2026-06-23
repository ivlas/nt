mod cli;
mod commands;
mod display;
mod error;
mod export;
mod fs;
mod index;
mod listing;
mod note;
mod query;
mod terminal;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    if let Err(err) = commands::run(cli) {
        let message = format!("error: {err}");
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

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

    if let Err(error) = commands::run(cli) {
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

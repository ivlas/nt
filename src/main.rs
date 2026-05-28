mod agent;
mod cli;
mod commands;
mod completion;
mod config;
mod error;
mod fs;
mod index;
mod note;
mod skills;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    if let Err(err) = commands::run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

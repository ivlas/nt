use clap::Parser;
use nt::{cli, commands, terminal};

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

use clap::CommandFactory;
use clap_complete::{Shell as ClapShell, generate};

use crate::cli::{Cli, Shell};

pub fn print_completion(shell: Shell) {
    let shell = match shell {
        Shell::Bash => ClapShell::Bash,
        Shell::Zsh => ClapShell::Zsh,
    };

    let mut command = Cli::command();
    generate(shell, &mut command, "nt", &mut std::io::stdout());
}

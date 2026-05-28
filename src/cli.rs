use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "Small note-taking CLI",
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Init { notes_dir: PathBuf },
    Add,
    List,
    Show { id: String },
    Edit { id: String },
    Find { query: String },
    Ids,
    Tags,
    Rebuild,
    Rm { id: String },
    Completion { shell: Shell },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    #[value(name = "powershell", alias = "power-shell")]
    PowerShell,
    Zsh,
}

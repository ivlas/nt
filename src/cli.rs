use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::config::AgentOutputMode;

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
    Init {
        notes_dir: PathBuf,
    },
    Add,
    List,
    Show {
        id: String,
    },
    Edit {
        id: String,
    },
    Find {
        query: String,
    },
    Ids,
    Tags,
    Rebuild,
    Rm {
        id: String,
    },
    Completion {
        shell: Shell,
    },
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Agent {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        prompt: Vec<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    #[value(name = "powershell", alias = "power-shell")]
    Power,
    Zsh,
}

#[derive(Subcommand)]
pub enum SkillCommand {
    Install,
    List,
    Show { name: String },
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    AgentOutput { mode: AgentOutputMode },
}

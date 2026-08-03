use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

pub mod help;

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "Local agent-first knowledge and memory layer",
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Init {
        vault: String,
    },
    Note {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        metadata: Vec<String>,
    },
    Todo {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        metadata: Vec<String>,
    },
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Find {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        expr: Vec<String>,
    },
    Show {
        id: String,
    },
    Open {
        id: String,
    },
    Rm {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    Update {
        id: String,
        field: UpdateField,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Agenda {
        view: Option<AgendaView>,
    },
    Export {
        path: PathBuf,
        ids: Vec<String>,
    },
    Help {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        topic: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum UpdateField {
    Kind,
    Status,
    Priority,
    Scheduled,
    Due,
    Tag,
    Collection,
    Home,
    Link,
    Source,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AgendaView {
    Today,
    Week,
    Overdue,
    Waiting,
    Undated,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{AgendaView, Cli, Command, UpdateField};

    const ID: &str = "018fbe0a-6c00-7000-8000-000000000001";

    #[test]
    fn parses_public_command_surface() {
        let cases: &[&[&str]] = &[
            &["nt", "init", "personal"],
            &["nt", "note", "home:personal/rust"],
            &["nt", "todo", "priority:A"],
            &["nt", "list", "id,title,home"],
            &["nt", "find", "body:ownership"],
            &["nt", "show", ID],
            &["nt", "open", ID],
            &["nt", "rm", ID],
            &["nt", "update", ID, "home", "work/project_a"],
            &["nt", "agenda", "week"],
            &["nt", "export", "archive", ID],
            &["nt", "help", "find"],
        ];
        for case in cases {
            Cli::try_parse_from(*case).unwrap_or_else(|error| panic!("{case:?}: {error}"));
        }
    }

    #[test]
    fn routes_typed_arguments() {
        let cli = Cli::parse_from(["nt", "init", "personal"]);
        assert!(matches!(cli.command, Some(Command::Init { vault }) if vault == "personal"));

        let cli = Cli::parse_from(["nt", "update", ID, "home", "work/project_a"]);
        assert!(matches!(
            cli.command,
            Some(Command::Update { id, field: UpdateField::Home, value })
                if id == ID && value == "work/project_a"
        ));

        let cli = Cli::parse_from(["nt", "agenda", "week"]);
        assert!(matches!(
            cli.command,
            Some(Command::Agenda {
                view: Some(AgendaView::Week)
            })
        ));
    }

    #[test]
    fn grammar_rejects_removed_and_flag_forms() {
        assert!(Cli::try_parse_from(["nt", "config"]).is_err());
        assert!(Cli::try_parse_from(["nt", "config", "show"]).is_err());
        assert!(Cli::try_parse_from(["nt", "config", "vault"]).is_err());
        assert!(Cli::try_parse_from(["nt", "rm"]).is_err());
        assert!(Cli::try_parse_from(["nt", "--help"]).is_err());
        assert!(Cli::parse_from(["nt"]).command.is_none());
    }

    #[test]
    fn command_names_are_stable() {
        let command = Cli::command();
        let commands: Vec<_> = command
            .get_subcommands()
            .map(|command| command.get_name())
            .collect();
        assert_eq!(
            commands,
            [
                "init", "note", "todo", "list", "find", "show", "open", "rm", "update", "agenda",
                "export", "help",
            ]
        );
    }
}

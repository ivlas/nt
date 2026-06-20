use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "Small CLI note organizer",
    disable_help_subcommand = true,
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
    Add {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        metadata: Vec<String>,
    },
    Rebuild,
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
        id: String,
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
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Completion {
        shell: Shell,
    },
    Help {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        topic: Vec<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LinkDirection {
    From,
    To,
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

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Vault { name: Option<String> },
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{AgendaView, Cli, Command, ConfigCommand, UpdateField};

    #[test]
    fn parses_target_commands() {
        let cases: &[&[&str]] = &[
            &["nt", "init", "notes"],
            &["nt", "add"],
            &["nt", "add", "tag:decision", "kind:note", "status:open"],
            &["nt", "rebuild"],
            &["nt", "list"],
            &["nt", "list", "ids"],
            &["nt", "list", "titles"],
            &["nt", "list", "tags"],
            &["nt", "list", "tags", "decision"],
            &["nt", "list", "collections"],
            &["nt", "list", "collections", "projects/nt"],
            &["nt", "list", "links", "NT20260528T143012", "from"],
            &["nt", "find", "tag:decision", "qemu"],
            &["nt", "show", "NT20260528T143012"],
            &["nt", "open", "NT20260528T143012"],
            &["nt", "rm", "NT20260528T143012"],
            &["nt", "update", "NT20260528T143012", "status", "open"],
            &["nt", "update", "NT20260528T143012", "tag", "+decision"],
            &["nt", "agenda"],
            &["nt", "agenda", "week"],
            &["nt", "export", "archive"],
            &["nt", "export", "archive", "NT20260528T143012"],
            &[
                "nt",
                "export",
                "archive",
                "NT20260528T143012",
                "NT20260527T120000",
            ],
            &["nt", "config", "show"],
            &["nt", "config", "vault"],
            &["nt", "config", "vault", "notes"],
            &["nt", "completion", "zsh"],
            &["nt", "help"],
            &["nt", "help", "find"],
        ];

        for case in cases {
            Cli::try_parse_from(*case).unwrap_or_else(|err| {
                panic!("failed to parse {case:?}: {err}");
            });
        }
    }

    #[test]
    fn find_uses_trailing_positionals() {
        let cli = Cli::parse_from(["nt", "find", "body:microvm jailer", "-hyphen"]);
        match cli.command {
            Command::Find { expr } => {
                assert_eq!(expr, vec!["body:microvm jailer", "-hyphen"]);
            }
            _ => panic!("expected find"),
        }
    }

    #[test]
    fn help_surface_matches_target_commands() {
        let command = Cli::command();
        let commands: Vec<&str> = command
            .get_subcommands()
            .map(|command| command.get_name())
            .collect();

        assert_eq!(
            commands,
            vec![
                "init",
                "add",
                "rebuild",
                "list",
                "find",
                "show",
                "open",
                "rm",
                "update",
                "agenda",
                "export",
                "config",
                "completion",
                "help",
            ]
        );

        assert!(Cli::try_parse_from(["nt", "--help"]).is_err());
        assert!(Cli::try_parse_from(["nt", "help"]).is_ok());

        let Command::Config {
            command: ConfigCommand::Vault { name },
        } = Cli::parse_from(["nt", "config", "vault", "notes"]).command
        else {
            panic!("expected config vault");
        };
        assert_eq!(name.as_deref(), Some("notes"));
    }

    #[test]
    fn typed_target_arguments_parse() {
        let cli = Cli::parse_from(["nt", "update", "NT20260528T143012", "priority", "S"]);
        assert!(matches!(
            cli.command,
            Command::Update {
                field: UpdateField::Priority,
                ..
            }
        ));
        let cli = Cli::parse_from(["nt", "agenda", "today"]);
        assert!(matches!(
            cli.command,
            Command::Agenda {
                view: Some(AgendaView::Today)
            }
        ));
        let cli = Cli::parse_from(["nt", "list", "id,title", "status:open"]);
        assert!(matches!(
            cli.command,
            Command::List { args }
                if args == ["id,title", "status:open"]
        ));
    }

    #[test]
    fn legacy_commands_are_unknown() {
        for command in [
            "ids",
            "tags",
            "collections",
            "collection",
            "links",
            "tag",
            "status",
            "link",
        ] {
            assert!(Cli::try_parse_from(["nt", command]).is_err());
        }
    }
}

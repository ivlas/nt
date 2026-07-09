use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

pub mod completion;
pub mod help;

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
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Init {
        notes_dir: PathBuf,
    },
    Note {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        metadata: Vec<String>,
    },
    Todo {
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

    use super::{AgendaView, Cli, Command, ConfigCommand, Shell, UpdateField};

    #[test]
    fn parses_target_commands() {
        let cases: &[&[&str]] = &[
            &["nt", "init", "notes"],
            &["nt", "note"],
            &["nt", "note", "tag:decision", "collection:projects/nt"],
            &["nt", "todo", "status:open", "priority:A"],
            &["nt", "rebuild"],
            &["nt", "list"],
            &["nt", "list", "ids"],
            &["nt", "list", "titles"],
            &["nt", "list", "tags"],
            &["nt", "list", "tags", "decision"],
            &["nt", "list", "collections"],
            &["nt", "list", "collections", "projects/nt"],
            &["nt", "list", "sources"],
            &["nt", "list", "sources", "https://example.com"],
            &["nt", "list", "links", "from:NT20260528T143012"],
            &["nt", "find", "tag:decision", "qemu"],
            &["nt", "show", "NT20260528T143012"],
            &["nt", "open", "NT20260528T143012"],
            &["nt", "rm", "NT20260528T143012"],
            &["nt", "rm", "NT20260528T143012", "NT20260527T120000"],
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
    fn target_commands_route_to_correct_variants() {
        let cli = Cli::parse_from(["nt", "init", "notes"]);
        assert!(matches!(
            cli.command,
            Some(Command::Init { notes_dir }) if notes_dir == std::path::Path::new("notes")
        ));

        let cli = Cli::parse_from(["nt", "note", "tag:decision", "collection:projects/nt"]);
        assert!(matches!(
            cli.command,
            Some(Command::Note { metadata }) if metadata == vec!["tag:decision", "collection:projects/nt"]
        ));

        let cli = Cli::parse_from(["nt", "todo", "status:open", "priority:A"]);
        assert!(matches!(
            cli.command,
            Some(Command::Todo { metadata }) if metadata == vec!["status:open", "priority:A"]
        ));

        let cli = Cli::parse_from(["nt", "rebuild"]);
        assert!(matches!(cli.command, Some(Command::Rebuild)));

        let cli = Cli::parse_from(["nt", "list"]);
        assert!(matches!(cli.command, Some(Command::List { args }) if args.is_empty()));

        let cli = Cli::parse_from(["nt", "list", "tags", "decision"]);
        assert!(matches!(
            cli.command,
            Some(Command::List { args }) if args == vec!["tags", "decision"]
        ));

        let cli = Cli::parse_from(["nt", "list", "links", "from:NT20260528T143012"]);
        assert!(matches!(
            cli.command,
            Some(Command::List { args }) if args == vec!["links", "from:NT20260528T143012"]
        ));

        let cli = Cli::parse_from(["nt", "show", "NT20260528T143012"]);
        assert!(matches!(
            cli.command,
            Some(Command::Show { id }) if id == "NT20260528T143012"
        ));

        let cli = Cli::parse_from(["nt", "open", "NT20260528T143012"]);
        assert!(matches!(
            cli.command,
            Some(Command::Open { id }) if id == "NT20260528T143012"
        ));

        let cli = Cli::parse_from(["nt", "update", "NT20260528T143012", "status", "open"]);
        assert!(matches!(
            cli.command,
            Some(Command::Update { id, field: UpdateField::Status, value })
                if id == "NT20260528T143012" && value == "open"
        ));

        let cli = Cli::parse_from(["nt", "update", "NT20260528T143012", "tag", "+decision"]);
        assert!(matches!(
            cli.command,
            Some(Command::Update { id, field: UpdateField::Tag, value })
                if id == "NT20260528T143012" && value == "+decision"
        ));

        let cli = Cli::parse_from(["nt", "agenda"]);
        assert!(matches!(cli.command, Some(Command::Agenda { view: None })));

        let cli = Cli::parse_from(["nt", "agenda", "week"]);
        assert!(matches!(
            cli.command,
            Some(Command::Agenda {
                view: Some(AgendaView::Week)
            })
        ));

        let cli = Cli::parse_from(["nt", "export", "archive"]);
        assert!(matches!(
            cli.command,
            Some(Command::Export { path, ids }) if path == std::path::Path::new("archive") && ids.is_empty()
        ));

        let cli = Cli::parse_from([
            "nt",
            "export",
            "archive",
            "NT20260528T143012",
            "NT20260527T120000",
        ]);
        assert!(matches!(
            cli.command,
            Some(Command::Export { path, ids })
                if path == std::path::Path::new("archive")
                    && ids == vec!["NT20260528T143012", "NT20260527T120000"]
        ));

        let cli = Cli::parse_from(["nt", "config", "show"]);
        assert!(matches!(
            cli.command,
            Some(Command::Config {
                command: ConfigCommand::Show
            })
        ));

        let cli = Cli::parse_from(["nt", "completion", "zsh"]);
        assert!(matches!(
            cli.command,
            Some(Command::Completion { shell: Shell::Zsh })
        ));

        let cli = Cli::parse_from(["nt", "help", "find"]);
        assert!(matches!(
            cli.command,
            Some(Command::Help { topic }) if topic == vec!["find"]
        ));
    }

    #[test]
    fn find_uses_trailing_positionals() {
        let cli = Cli::parse_from(["nt", "find", "body:microvm jailer", "-hyphen"]);
        match cli.command {
            Some(Command::Find { expr }) => {
                assert_eq!(expr, vec!["body:microvm jailer", "-hyphen"]);
            }
            _ => panic!("expected find"),
        }
    }

    #[test]
    fn rm_requires_one_or_more_ids() {
        assert!(Cli::try_parse_from(["nt", "rm"]).is_err());

        let cli = Cli::parse_from(["nt", "rm", "NT20260528T143012", "NT20260527T120000"]);
        assert!(matches!(
            cli.command,
            Some(Command::Rm { ids })
                if ids == ["NT20260528T143012", "NT20260527T120000"]
        ));
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
                "note",
                "todo",
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

        let Some(Command::Config {
            command: ConfigCommand::Vault { name },
        }) = Cli::parse_from(["nt", "config", "vault", "notes"]).command
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
            Some(Command::Update {
                field: UpdateField::Priority,
                ..
            })
        ));
        let cli = Cli::parse_from(["nt", "agenda", "today"]);
        assert!(matches!(
            cli.command,
            Some(Command::Agenda {
                view: Some(AgendaView::Today)
            })
        ));
        let cli = Cli::parse_from(["nt", "list", "id,title", "status:open"]);
        assert!(matches!(
            cli.command,
            Some(Command::List { args })
                if args == ["id,title", "status:open"]
        ));
    }

    #[test]
    fn empty_invocation_parses_without_a_subcommand() {
        assert!(Cli::parse_from(["nt"]).command.is_none());
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

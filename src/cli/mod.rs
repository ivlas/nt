use clap::{Parser, Subcommand};

pub mod help;
pub(crate) mod input;
pub(crate) mod output;
pub(crate) mod paths;
pub(crate) mod rendering;
pub(crate) mod terminal;

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "Local agent-first notes",
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
    Init,
    Add {
        metadata: Vec<String>,
        #[arg(last = true, allow_hyphen_values = true)]
        body: Vec<String>,
    },
    Show {
        id: String,
    },
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        filters: Vec<String>,
    },
    Read {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        filters: Vec<String>,
    },
    Changes {
        cursor: String,
    },
    Find {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        expressions: Vec<String>,
    },
    Rm {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    Edit {
        id: String,
        #[arg(last = true, allow_hyphen_values = true)]
        body: Vec<String>,
    },
    Move {
        id: String,
        collection: String,
    },
    Tag {
        id: String,
        #[arg(allow_hyphen_values = true)]
        operation: String,
    },
    Link {
        id: String,
        #[arg(allow_hyphen_values = true)]
        operation: String,
    },
    Help {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        topic: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command};

    const ID: &str = "018fbe0a-6c00-7000-8000-000000000001";

    #[test]
    fn parses_clean_sheet_command_surface() {
        let cases: &[&[&str]] = &[
            &["nt", "init"],
            &[
                "nt",
                "add",
                "collection:work/nt",
                "tag:rust",
                "--",
                "# Note",
            ],
            &["nt", "show", ID],
            &["nt", "list", "tag:rust"],
            &["nt", "list", "tags"],
            &["nt", "list", "collections"],
            &["nt", "read", "tag:rust"],
            &["nt", "changes", "since:0"],
            &["nt", "find", "sqlite", "tag:rust"],
            &["nt", "rm", ID],
            &["nt", "edit", ID, "--", "# Updated"],
            &["nt", "move", ID, "work/nt"],
            &["nt", "tag", ID, "+rust"],
            &["nt", "link", ID, "+018fbe0a-6c00-7000-8000-000000000002"],
            &["nt", "help", "find"],
        ];
        for case in cases {
            Cli::try_parse_from(*case).unwrap_or_else(|error| panic!("{case:?}: {error}"));
        }
    }

    #[test]
    fn separates_capture_metadata_from_trailing_body() {
        let cli = Cli::parse_from([
            "nt",
            "add",
            "tag:rust",
            "--",
            "# Storage",
            "collection:not-metadata",
        ]);
        assert!(matches!(
            cli.command,
            Some(Command::Add { metadata, body })
                if metadata == ["tag:rust"]
                    && body == ["# Storage", "collection:not-metadata"]
        ));
    }

    #[test]
    fn rejects_removed_and_flag_forms() {
        for removed in [
            "note", "memory", "todo", "update", "agenda", "export", "config",
        ] {
            assert!(Cli::try_parse_from(["nt", removed]).is_err());
        }
        assert!(Cli::try_parse_from(["nt", "init", "personal"]).is_err());
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
                "init", "add", "show", "list", "read", "changes", "find", "rm", "edit", "move",
                "tag", "link", "help",
            ]
        );
    }
}

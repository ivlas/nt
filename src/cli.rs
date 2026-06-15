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
    List,
    Find {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        expr: Vec<String>,
    },
    Show {
        id: String,
    },
    Edit {
        id: String,
    },
    Rm {
        id: String,
    },
    Ids,
    Tags,
    Tag {
        id: String,
        tag: String,
    },
    Untag {
        id: String,
        tag: String,
    },
    Collections,
    Collection {
        name: String,
    },
    Collect {
        id: String,
        collection: String,
    },
    Uncollect {
        id: String,
        collection: String,
    },
    Kind {
        id: String,
        kind: String,
    },
    Status {
        #[arg(num_args = 2)]
        args: Vec<String>,
    },
    Link {
        from_id: String,
        to_id: String,
    },
    Unlink {
        from_id: String,
        to_id: String,
    },
    Links {
        id: String,
        mode: LinkMode,
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

#[derive(Clone, Copy, ValueEnum)]
pub enum LinkMode {
    Out,
    In,
    #[value(name = "self")]
    Self_,
    All,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    Vault { name: Option<String> },
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, ConfigCommand};

    #[test]
    fn parses_documented_v1_commands() {
        let cases: &[&[&str]] = &[
            &["nt", "init", "notes"],
            &["nt", "add"],
            &["nt", "add", "tag:decision", "kind:note", "status:open"],
            &["nt", "rebuild"],
            &["nt", "list"],
            &["nt", "find", "tag:decision", "qemu"],
            &["nt", "show", "NT20260528T143012"],
            &["nt", "edit", "NT20260528T143012"],
            &["nt", "rm", "NT20260528T143012"],
            &["nt", "ids"],
            &["nt", "tags"],
            &["nt", "tag", "NT20260528T143012", "decision"],
            &["nt", "untag", "NT20260528T143012", "decision"],
            &["nt", "collections"],
            &["nt", "collection", "projects/nt"],
            &["nt", "collect", "NT20260528T143012", "projects/nt"],
            &["nt", "uncollect", "NT20260528T143012", "projects/nt"],
            &["nt", "kind", "NT20260528T143012", "decision"],
            &["nt", "status"],
            &["nt", "status", "NT20260528T143012", "open"],
            &["nt", "link", "NT20260528T143012", "NT20260527T120000"],
            &["nt", "unlink", "NT20260528T143012", "NT20260527T120000"],
            &["nt", "links", "NT20260528T143012", "out"],
            &["nt", "links", "NT20260528T143012", "in"],
            &["nt", "links", "NT20260528T143012", "self"],
            &["nt", "links", "NT20260528T143012", "all"],
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
    fn help_surface_matches_documented_v1_commands() {
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
                "edit",
                "rm",
                "ids",
                "tags",
                "tag",
                "untag",
                "collections",
                "collection",
                "collect",
                "uncollect",
                "kind",
                "status",
                "link",
                "unlink",
                "links",
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
    fn status_accepts_zero_or_two_positionals() {
        assert!(Cli::try_parse_from(["nt", "status"]).is_ok());
        assert!(Cli::try_parse_from(["nt", "status", "NT20260528T143012", "open"]).is_ok());
        assert!(Cli::try_parse_from(["nt", "status", "NT20260528T143012"]).is_err());
    }
}

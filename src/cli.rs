use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::config::AgentOutputMode;

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "Small CLI note organizer and research workspace",
    disable_help_subcommand = true,
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
    Discuss {
        id: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        prompt: Vec<String>,
    },
    Rm {
        id: String,
    },
    Rebuild,
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
    },
    Backlinks {
        id: String,
    },
    Agent {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        prompt: Vec<String>,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Completion {
        shell: Shell,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    Show,
    AgentOutput { mode: AgentOutputMode },
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
            &["nt", "list"],
            &["nt", "find", "tag:decision", "qemu"],
            &["nt", "show", "NT20260528T143012"],
            &["nt", "edit", "NT20260528T143012"],
            &["nt", "discuss", "NT20260528T143012"],
            &["nt", "discuss", "NT20260528T143012", "what", "changed?"],
            &["nt", "rm", "NT20260528T143012"],
            &["nt", "rebuild"],
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
            &["nt", "links", "NT20260528T143012"],
            &["nt", "backlinks", "NT20260528T143012"],
            &["nt", "agent", "summarize", "recent", "notes"],
            &["nt", "config", "show"],
            &["nt", "config", "agent-output", "hidden"],
            &["nt", "completion", "zsh"],
        ];

        for case in cases {
            Cli::try_parse_from(*case).unwrap_or_else(|err| {
                panic!("failed to parse {case:?}: {err}");
            });
        }
    }

    #[test]
    fn find_and_prompts_use_trailing_positionals() {
        let cli = Cli::parse_from(["nt", "find", "body:microvm jailer", "-hyphen"]);
        match cli.command {
            Command::Find { expr } => {
                assert_eq!(expr, vec!["body:microvm jailer", "-hyphen"]);
            }
            _ => panic!("expected find"),
        }

        let cli = Cli::parse_from(["nt", "agent", "explain", "NT20260528T143012"]);
        match cli.command {
            Command::Agent { prompt } => assert_eq!(prompt, vec!["explain", "NT20260528T143012"]),
            _ => panic!("expected agent"),
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
                "list",
                "find",
                "show",
                "edit",
                "discuss",
                "rm",
                "rebuild",
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
                "backlinks",
                "agent",
                "config",
                "completion",
            ]
        );

        assert!(Cli::try_parse_from(["nt", "help"]).is_err());

        let Command::Config {
            command: ConfigCommand::AgentOutput { .. },
        } = Cli::parse_from(["nt", "config", "agent-output", "full"]).command
        else {
            panic!("expected config agent-output");
        };
    }

    #[test]
    fn status_accepts_zero_or_two_positionals() {
        assert!(Cli::try_parse_from(["nt", "status"]).is_ok());
        assert!(Cli::try_parse_from(["nt", "status", "NT20260528T143012", "open"]).is_ok());
        assert!(Cli::try_parse_from(["nt", "status", "NT20260528T143012"]).is_err());
    }
}

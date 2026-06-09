use crate::error::{NtError, Result};

pub fn print(topic: &[String]) -> Result<()> {
    let key = topic.join(" ");
    let text = topic_text(&key)?;

    print!("{text}");
    Ok(())
}

fn topic_text(key: &str) -> Result<&'static str> {
    let text = match key {
        "" => ROOT,
        "init" => INIT,
        "add" => ADD,
        "list" => LIST,
        "find" => FIND,
        "show" => SHOW,
        "edit" => EDIT,
        "rm" => RM,
        "ids" => IDS,
        "tags" => TAGS,
        "tag" => TAG,
        "untag" => UNTAG,
        "collections" => COLLECTIONS,
        "collection" => COLLECTION,
        "collect" => COLLECT,
        "uncollect" => UNCOLLECT,
        "kind" => KIND,
        "status" => STATUS,
        "link" => LINK,
        "unlink" => UNLINK,
        "links" => LINKS,
        "export" => EXPORT,
        "config" => CONFIG,
        "config show" => CONFIG_SHOW,
        "config vault" => CONFIG_VAULT,
        "completion" => COMPLETION,
        "help" => HELP,
        _ => {
            return Err(NtError::Message(format!(
                "unknown help topic `{key}`; run `nt help`"
            )));
        }
    };

    Ok(text)
}

const ROOT: &str = r#"nt

Small CLI note organizer and research workspace.

Usage:
  nt <command> [positional...]
  nt help <command>

Commands:
  init         create a vault
  add          add a Markdown note
  list         list recent notes
  find         find notes by query expressions
  show         show one exact note
  edit         edit one note with $EDITOR
  rm           remove one note
  ids          print note ids
  tags         print known tags
  tag          add a tag
  untag        remove a tag
  collections  print known collections
  collection   list one collection
  collect      add a note to a collection
  uncollect    remove a note from a collection
  kind         set note kind
  status       list or set status
  link         add a note link
  unlink       remove a note link
  links        print note links
  export       export Markdown with front matter
  config       inspect or select vaults
  completion   generate shell completion
  help         show help

Examples:
  nt init notes
  nt add tag:decision kind:note
  nt find tag:decision qemu
  nt help find
"#;

const INIT: &str = r#"nt init <notes-dir>

Create a vault from a flat notes directory and make it active. The vault name is
the directory basename and must be unique.

Examples:
  nt init notes
  nt init ~/notes/nt
"#;

const ADD: &str = r#"nt add [metadata...]

Read a CommonMark note from stdin, or open $EDITOR when stdin is a terminal.
Metadata uses visible positional expressions.

Examples:
  nt add
  nt add tag:storage kind:decision status:open
  nt add tag:qemu,firecracker collection:research/qemu
"#;

const LIST: &str = r#"nt list

Print recent note summaries.

Examples:
  nt list
  nt list | head
"#;

const FIND: &str = r#"nt find <expr...>

Find notes with AND-combined query expressions. Bare words match searchable
metadata and note bodies.

Examples:
  nt find qemu firecracker
  nt find tag:decision collection:projects/nt
  nt find since:2026-05-01 before:2026-06-01 not:tag:draft
"#;

const SHOW: &str = r#"nt show <id>

Print note identity, metadata, and CommonMark body for one exact note.

Examples:
  nt show NT20260528T143012
  nt show NT20260528T143012 | sed -n '1,12p'
"#;

const EDIT: &str = r#"nt edit <id>

Open one note in $EDITOR and save the edited Markdown body.

Examples:
  nt edit NT20260528T143012
  EDITOR=vim nt edit NT20260528T143012
"#;

const RM: &str = r#"nt rm <id>

Remove one note file and its visible index metadata.

Examples:
  nt rm NT20260528T143012
"#;

const IDS: &str = r#"nt ids

Print note ids, one per line.

Examples:
  nt ids
  nt ids | head
"#;

const TAGS: &str = r#"nt tags

Print known tags and counts.

Examples:
  nt tags
  nt tags | sort
"#;

const TAG: &str = r#"nt tag <id> <tag>

Add a sparse topic tag to one note.

Examples:
  nt tag NT20260528T143012 storage
  nt tag NT20260528T143012 qemu
"#;

const UNTAG: &str = r#"nt untag <id> <tag>

Remove a tag from one note.

Examples:
  nt untag NT20260528T143012 draft
"#;

const COLLECTIONS: &str = r#"nt collections

Print known collection names, one per line.

Examples:
  nt collections
  nt collections | sort
"#;

const COLLECTION: &str = r#"nt collection <name>

List notes in one collection using the normal summary format.

Examples:
  nt collection projects/nt
  nt collection research/qemu
"#;

const COLLECT: &str = r#"nt collect <id> <collection>

Add one note to a workspace-like collection.

Examples:
  nt collect NT20260528T143012 projects/nt
  nt collect NT20260528T143012 research/qemu
"#;

const UNCOLLECT: &str = r#"nt uncollect <id> <collection>

Remove one note from a collection.

Examples:
  nt uncollect NT20260528T143012 projects/nt
"#;

const KIND: &str = r#"nt kind <id> <kind>

Set the structural form of a note.

Examples:
  nt kind NT20260528T143012 decision
  nt kind NT20260528T143012 meeting
"#;

const STATUS: &str = r#"nt status
nt status <id> <status>

List open and waiting notes, or set one note status.

Examples:
  nt status
  nt status NT20260528T143012 open
  nt status NT20260528T143012 done
"#;

const LINK: &str = r#"nt link <from-id> <to-id>

Add an exact note-to-note relationship in visible JSON metadata.

Examples:
  nt link NT20260528T143012 NT20260527T120000
"#;

const UNLINK: &str = r#"nt unlink <from-id> <to-id>

Remove an exact note-to-note relationship.

Examples:
  nt unlink NT20260528T143012 NT20260527T120000
"#;

const LINKS: &str = r#"nt links <id> <out|in|self|all>

Print outbound links, inbound links, direct neighbors, or a connected walk.

Examples:
  nt links NT20260528T143012 out
  nt links NT20260528T143012 in
  nt links NT20260528T143012 all
"#;

const EXPORT: &str = r#"nt export <path> [id...]

Export notes into a directory as Markdown files with generated front matter.
Metadata is read from the JSON index; active note files are not modified.

Examples:
  nt export archive
  nt export archive NT20260528T143012
  nt export archive NT20260528T143012 NT20260527T120000
  nt find collection:projects/nt | awk '{print $1}' | while read -r id; do nt export archive "$id"; done
"#;

const CONFIG: &str = r#"nt config show
nt config vault [vault-name]

Inspect vault state or select the active vault.

Examples:
  nt config show
  nt config vault
  nt config vault notes
"#;

const CONFIG_SHOW: &str = r#"nt config show

Print the active vault name and path.

Examples:
  nt config show
"#;

const CONFIG_VAULT: &str = r#"nt config vault [vault-name]

List known vaults, or select the active vault by name.

Examples:
  nt config vault
  nt config vault notes
"#;

const COMPLETION: &str = r#"nt completion <shell>

Generate shell completion for commands and dynamic note ids.

Examples:
  nt completion zsh
  nt completion bash
"#;

const HELP: &str = r#"nt help [command...]

Show short command help with examples.

Examples:
  nt help
  nt help find
  nt help config vault
"#;

#[cfg(test)]
mod tests {
    use super::topic_text;

    #[test]
    fn every_command_has_help_text() {
        let topics = [
            "",
            "init",
            "add",
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
            "config show",
            "config vault",
            "completion",
            "help",
        ];

        for topic in topics {
            let text = topic_text(topic).unwrap_or_else(|err| {
                panic!("missing help for {topic:?}: {err}");
            });
            assert!(text.contains("Examples:"), "missing examples for {topic:?}");
        }
    }

    #[test]
    fn unknown_help_topic_is_actionable() {
        let err = topic_text("unknown").unwrap_err();

        assert_eq!(
            err.to_string(),
            "unknown help topic `unknown`; run `nt help`"
        );
    }
}

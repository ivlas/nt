use crate::error::{NtError, Result};

pub fn print(topic: &[String]) -> Result<()> {
    print!("{}", topic_text(&topic.join(" "))?);
    Ok(())
}

fn topic_text(key: &str) -> Result<&'static str> {
    match key {
        "" => Ok(ROOT),
        "init" => Ok(
            "nt init <notes-dir>\n\nCreate and select a flat note vault.\n\nExamples:\n  nt init notes\n",
        ),
        "add" => Ok(ADD),
        "rebuild" => Ok(REBUILD),
        "list" => Ok(LIST),
        "find" => Ok(FIND),
        "show" => Ok(
            "nt show <id>\n\nPrint metadata and the CommonMark body.\n\nExamples:\n  nt show NT20260528T143012\n",
        ),
        "open" => Ok(
            "nt open <id>\n\nEdit one note with $EDITOR.\n\nExamples:\n  nt open NT20260528T143012\n",
        ),
        "rm" => Ok(RM),
        "update" => Ok(UPDATE),
        "agenda" => Ok(AGENDA),
        "export" => Ok(
            "nt export <path> [id...]\n\nExport Markdown with generated front matter.\n\nExamples:\n  nt export archive\n",
        ),
        "config" => Ok(CONFIG),
        "config show" => {
            Ok("nt config show\n\nPrint active vault state.\n\nExamples:\n  nt config show\n")
        }
        "config vault" => Ok(
            "nt config vault [vault-name]\n\nList or select vaults.\n\nExamples:\n  nt config vault notes\n",
        ),
        "completion" => Ok(
            "nt completion <bash|zsh>\n\nGenerate shell completion.\n\nExamples:\n  nt completion zsh\n",
        ),
        "help" => Ok(
            "nt help [command...]\nnt help reference\n\nShow command help or the compact CLI reference.\n\nExamples:\n  nt help find\n  nt help reference\n",
        ),
        "reference" => Ok(REFERENCE),
        _ => Err(NtError::Message(format!(
            "unknown help topic `{key}`; run `nt help`"
        ))),
    }
}

const ROOT: &str = r#"nt

Markdown-first CLI note organizer.

Usage:
  nt <command> [args...]

Getting started:
  init <notes-dir>                    create and select a vault
  add [metadata...]                   add a CommonMark note
  list [projection] [filter...]       list notes and metadata
  find <expr...>                      find notes by query expressions

Read and edit:
  show <id>                           show one exact note
  open <id>                           edit one note with $EDITOR
  rm <id...>                          remove one or more notes

Plan and organize:
  update <id> <field> <value>         update one metadata field
  agenda [today|week|overdue|waiting|undated]  show actionable todos

Maintenance:
  rebuild                             rebuild the active vault index
  export <path> [id...]               export Markdown with front matter
  config show                         inspect the active vault
  config vault [vault-name]           list or select vaults
  completion <bash|zsh>               generate shell completion

Help:
  help [command...]                   show command help
  help reference                      show the compact CLI reference

Examples:
  nt init notes
  nt add kind:todo status:open
  nt find tag:decision qemu
  nt show NT20260528T143012
"#;

const ADD: &str = r#"nt add [metadata...]

Read CommonMark from stdin or $EDITOR. Metadata fields are tag, kind, status,
priority, scheduled, due, collection, link, and source.

Examples:
  nt add kind:todo status:open priority:A due:2026-06-30
"#;

const REBUILD: &str = r#"nt rebuild

Rebuild from Markdown while preserving primary JSON metadata and merging URLs
currently found in Markdown bodies.

Examples:
  nt rebuild
"#;

const RM: &str = r#"nt rm <id...>

Remove one or more notes and update the index once.

Examples:
  nt rm NT20260528T143012
  nt rm NT20260528T143012 NT20260527T120000
"#;

const LIST: &str = r#"nt list
nt list all [filter...]
nt list <field>[,<field>...] [filter...]
nt list ids
nt list titles
nt list tags [tag]
nt list collections [collection]
nt list links [filter...]

Print active-vault metadata rows with optional structured filters. `list links`
prints one FROM/TO row per link, including both note titles. Use `from:<id>` and
`to:<id>` to select edge endpoints; other filters apply to the FROM note.
Fields include id, path, created, updated, title, kind, status, priority,
scheduled, due, closed, tag, collection, link, and source. Bare list prints id,
title, kind, status, due, and tag; `all` prints every field. `link:<id>` filters
notes that link to that id. Positional link directions and directionless
`list links <id>` are not supported.

Examples:
  nt list
  nt list all status:done
  nt list id
  nt list id,title,status status:open
  nt list title,tag collection:projects/nt
  nt list id,title link:NT20260528T143012
  nt list tags storage
  nt list collections projects/nt
  nt list links
  nt list links day:2026-06-20
  nt list links from:NT20260528T143012
  nt list links to:NT20260528T143012 status:open
"#;

const FIND: &str = r#"nt find <expr...>

Find notes with AND-combined expressions. Fields include id, tag, title, day,
since, before, kind, status, priority, scheduled, due, closed, collection,
link, source, and body.

Examples:
  nt find kind:todo due:2026-06-30
  nt find not:status:done qemu
"#;

const UPDATE: &str = r#"nt update <id> <field> <value>

Single fields kind, status, priority, scheduled, and due use a value or -.
Set fields tag, collection, link, and source require +value or -value.

Examples:
  nt update NT20260528T143012 status done
  nt update NT20260528T143012 tag +decision
"#;

const AGENDA: &str = r#"nt agenda [today|week|overdue|waiting|undated]

Print actionable todo records ordered by date, priority, and recency.

Examples:
  nt agenda
  nt agenda week
"#;

const CONFIG: &str = r#"nt config show
nt config vault [vault-name]

Inspect or select vaults.

Examples:
  nt config show
  nt config vault notes
"#;

const REFERENCE: &str = r#"nt CLI reference

Commands:
  nt
  nt init <notes-dir>
  nt add [metadata...]
  nt rebuild
  nt list [projection] [filter...]
  nt find <expr...>
  nt show <id>
  nt open <id>
  nt rm <id...>
  nt update <id> <field> <value>
  nt agenda [today|week|overdue|waiting|undated]
  nt export <path> [id...]
  nt config show
  nt config vault [vault-name]
  nt completion <bash|zsh>
  nt help [command...]
  nt help reference

Add metadata:
  kind:<kind> status:<status> priority:<priority>
  scheduled:<date> due:<date>
  tag:<tag>[,<tag>...] collection:<name>[,<name>...]
  link:<id>[,<id>...] source:<value>
  Set metadata is repeatable; commas in source values are literal.
  Example:
    printf '%s\n' '# Research' '' 'Compare runtimes.' |
      nt add kind:research tag:qemu collection:research/vm

List:
  projections  all | <field>[,<field>...]
  fields       id path created updated title kind status priority scheduled
               due closed tag collection link source
  modes        ids | titles | tags [tag] | collections [name] | links
  filters      id:<prefix> tag:<tag> day:<date> since:<date> before:<date>
               kind:<kind> status:<status> priority:<priority>
               scheduled:<date> due:<date> closed:<date>
               collection:<name> link:<id> not:<filter>
  link edges   from:<id> to:<id> (with `nt list links`)

Find:
  <word> #<tag> id:<prefix> tag:<tag> title:<term>
  day:<date> since:<date> before:<date> kind:<kind> status:<status>
  priority:<priority> scheduled:<date> due:<date> closed:<date>
  collection:<name> link:<id> source:<term> body:<term> not:<expr>
  Expressions are case-insensitive and AND-combined.

Update:
  single fields  kind status priority scheduled due
                 use <value>; use - to clear
  set fields     tag collection link source
                 use +<value> or -<value>
  closed is system-managed; clearing kind resets it to note.

Values:
  id          NTYYYYMMDDTHHmmss
  date        YYYY-MM-DD
  kind        note todo meeting decision source research project
  status      open waiting done dropped
  priority    S A B C D
  tag/name    lowercase, no whitespace or commas

Rules:
  `nt` with no arguments prints the same output as `nt help`.
  `add` reads CommonMark from stdin or opens $EDITOR; `open` uses $EDITOR.
  Links target existing active notes. Dates are valid calendar dates.
  Core workflows are positional; use `nt help`, not `--help`.
  Use shell quoting for spaces: body:'microvm jailer'.
  Multiword body values match all terms, not an exact phrase.
  Successful mutations print one short line; records are one per line.
  Errors go to stderr. Run `nt help <command>` for details.
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};

    #[test]
    fn target_commands_have_help() {
        for topic in [
            "",
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
        ] {
            assert!(topic_text(topic).unwrap().contains("Examples:"));
        }
    }

    #[test]
    fn reference_covers_operational_grammar() {
        let reference = topic_text("reference").unwrap();

        for section in [
            "Commands:",
            "Add metadata:",
            "List:",
            "Find:",
            "Update:",
            "Values:",
            "Rules:",
        ] {
            assert!(reference.contains(section));
        }

        for syntax in [
            "nt rm <id...>",
            "nt list [projection] [filter...]",
            "body:<term>",
            "not:<expr>",
            "+<value> or -<value>",
            "nt add kind:research tag:qemu collection:research/vm",
            "NTYYYYMMDDTHHmmss",
            "YYYY-MM-DD",
        ] {
            assert!(reference.contains(syntax));
        }
    }

    #[test]
    fn legacy_topics_are_unknown() {
        for topic in ["ids", "tags", "collection", "status", "link"] {
            assert!(topic_text(topic).is_err());
        }
    }

    #[test]
    fn root_help_groups_current_commands_and_shows_argument_shapes() {
        for heading in [
            "Getting started:",
            "Read and edit:",
            "Plan and organize:",
            "Maintenance:",
            "Help:",
        ] {
            assert!(ROOT.contains(heading));
        }

        for usage in [
            "init <notes-dir>",
            "add [metadata...]",
            "list [projection] [filter...]",
            "find <expr...>",
            "show <id>",
            "open <id>",
            "rm <id...>",
            "update <id> <field> <value>",
            "agenda [today|week|overdue|waiting|undated]",
            "export <path> [id...]",
            "config show",
            "config vault [vault-name]",
            "completion <bash|zsh>",
            "help [command...]",
            "help reference",
        ] {
            assert!(ROOT.contains(usage));
        }
    }
}

use crate::error::{NtError, Result};

pub fn print(topic: &[String]) -> Result<()> {
    print!("{}", topic_text(&topic.join(" "))?);
    Ok(())
}

fn topic_text(key: &str) -> Result<&'static str> {
    match key {
        "" => Ok(ROOT),
        "init" => Ok(
            "nt init <vault>\n\nCreate a logical vault and its inbox collection.\n\nExamples:\n  nt init personal\n",
        ),
        "note" => Ok(NOTE),
        "todo" => Ok(TODO),
        "list" => Ok(LIST),
        "find" => Ok(FIND),
        "show" => Ok(
            "nt show <id>\n\nPrint kind-specific metadata and the CommonMark body.\n\nExamples:\n  nt show 018fbe0a-6c00-7000-8000-000000000001\n",
        ),
        "rm" => Ok(RM),
        "update" => Ok(UPDATE),
        "agenda" => Ok(AGENDA),
        "export" => Ok(
            "nt export <path> [id...]\n\nExport portable Markdown snapshots with generated front matter.\n\nExamples:\n  nt export archive\n",
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

Local, agent-first knowledge and memory layer.

Usage:
  nt <command> [args...]

Getting started:
  init <vault>                        create a logical vault
  note [metadata...]                  add a CommonMark note
  todo [metadata...]                  add an actionable todo
  list [projection] [filter...]       list notes and metadata
  find <expr...>                      retrieve notes by query expressions

Read and remove:
  show <id>                           show one exact note
  rm <id...>                          remove one or more notes

Plan and organize:
  update <id> body                    replace CommonMark from stdin or $EDITOR
  update <id> <field> <value>         update one metadata field
  agenda [week]                       show dated open todos

Maintenance:
  export <path> [id...]               export Markdown snapshots

Help:
  help [command...]                   show command help
  help reference                      show the compact CLI reference

Examples:
  nt init personal
  nt note home:personal/rust
  nt find tag:decision qemu
  nt agenda week
"#;

const NOTE: &str = r#"nt note [metadata...]

Read CommonMark from stdin or $EDITOR. Metadata fields are home, tag,
collection, link, and source. Collections use <vault>/<collection>. The first
collection is home unless home is explicit. With one vault, the default is its
inbox; with multiple vaults, home is required.

Examples:
  nt note home:personal/rust collection:work/project_a tag:storage
"#;

const TODO: &str = r#"nt todo [metadata...]

Create a kind:todo note. New todos default to status:open. In addition to note
metadata, todo accepts status, priority, scheduled, and due.

Examples:
  nt todo home:work/project_a priority:A due:2026-06-30
"#;

const RM: &str = r#"nt rm <id...>

Remove one or more notes and their relationships in one transaction.

Examples:
  nt rm 018fbe0a-6c00-7000-8000-000000000001
"#;

const LIST: &str = r#"nt list
nt list all [filter...]
nt list <field>[,<field>...] [filter...]

Fields are id, home, created, updated, title, kind, status, priority, scheduled,
due, closed, tag, collection, link, and source. Redirected projections are
stable tab-separated rows, one per note. Set-valued fields are comma-separated.
Filters are structured metadata expressions and are AND-combined. Use all to
select every field.

Examples:
  nt list id,title,home collection:personal/rust
  nt list tag
  nt list id,title,status,priority,scheduled,due kind:todo status:open
"#;

const FIND: &str = r#"nt find <expr...>

Find notes with AND-combined expressions. Bare, title, and body values require
all whole Unicode tokens in any order; punctuation separates terms, case is
folded, supported Latin diacritics are removed, and prefixes are not expanded.
Source uses a case-insensitive SQL substring. Fields are id, tag, title, day,
since, before, kind, status, priority, scheduled, due, closed, collection, link,
source, and body. Results are ordered by recency, not relevance.

Examples:
  nt find kind:todo due:2026-06-30
  nt find collection:personal/rust body:'ownership borrow'
"#;

const UPDATE: &str = r#"nt update <id> body
nt update <id> <field> <value>

Body replaces the complete CommonMark document from stdin or $EDITOR and
rederives the title. Single metadata fields kind, status, priority, scheduled,
and due use a value or -. Home takes a fully qualified collection. Set fields
tag, collection, link, and source require +value or -value. A home membership
cannot be removed until home is moved.

Examples:
  printf '%s\n' '# Updated' '' 'Replacement body.' | nt update 018fbe0a-6c00-7000-8000-000000000001 body
  nt update 018fbe0a-6c00-7000-8000-000000000001 home work/project_a
  nt update 018fbe0a-6c00-7000-8000-000000000001 tag +decision
"#;

const AGENDA: &str = r#"nt agenda [week]

Print open todos that need attention by date. The default includes overdue and
today; week also includes the next six days. Waiting, undated, done, and dropped
todos are excluded. Rows contain id, priority, scheduled, due, and title.

Examples:
  nt agenda
  nt agenda week
"#;

const REFERENCE: &str = r#"nt CLI reference

Commands:
  nt init <vault>
  nt note [metadata...]
  nt todo [metadata...]
  nt list [projection] [filter...]
  nt find <expr...>
  nt show <id>
  nt rm <id...>
  nt update <id> body
  nt update <id> <field> <value>
  nt agenda [week]
  nt export <path> [id...]
  nt help [command...]

Note metadata:
  home:<vault>/<collection>
  tag:<tag>[,<tag>...] collection:<vault>/<collection>[,...]
  link:<id>[,<id>...] source:<value>

Todo metadata:
  status:<status> priority:<priority> scheduled:<date> due:<date>
  plus all note metadata; new todos default to status:open.

List:
  fields       id home created updated title kind status priority scheduled
               due closed tag collection link source
  filters      id:<prefix> tag:<tag> day:<date> since:<date> before:<date>
               kind:<kind> status:<status> priority:<priority>
               scheduled:<date> due:<date> closed:<date>
               collection:<name> link:<id> not:<filter>

Find:
  <word> #<tag> id:<prefix> tag:<tag> title:<term> body:<term>
  collection:<name> link:<id> source:<term> not:<expr>
  Expressions are AND-combined. Bare/title/body use whole Unicode tokens;
  source remains a case-insensitive SQL substring.

Update:
  body           replace CommonMark from stdin or $EDITOR
  single fields  kind status priority scheduled due; use - to clear
  home field     fully qualified collection
  set fields     tag collection link source; use +<value> or -<value>

Values:
  id          canonical UUIDv7
  collection  <vault>/<collection>
  date        YYYY-MM-DD
  kind        note todo
  status      open waiting done dropped
  priority    S A B C D

Rules:
  SQLite at $HOME/.nt/nt.sqlite3 is canonical for bodies and metadata.
  Logical vaults are namespaces, not directories; there is no active vault.
  Notes may reference collections in multiple vaults and have exactly one home.
  Core workflows are positional; use `nt help`, not `--help`.
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};

    #[test]
    fn all_commands_have_examples() {
        for topic in [
            "", "init", "note", "todo", "list", "find", "show", "rm", "update", "agenda", "export",
            "help",
        ] {
            assert!(topic_text(topic).unwrap().contains("Examples:"));
        }
    }

    #[test]
    fn help_describes_sqlite_uuid_and_logical_vaults() {
        let reference = topic_text("reference").unwrap();
        for term in [
            "SQLite",
            "UUIDv7",
            "home:<vault>/<collection>",
            "no active vault",
        ] {
            assert!(reference.contains(term));
        }
        assert!(ROOT.contains("init <vault>"));
    }

    #[test]
    fn unknown_topics_are_errors() {
        assert!(topic_text("search").is_err());
    }
}

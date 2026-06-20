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
        "rm" => Ok("nt rm <id>\n\nRemove one note.\n\nExamples:\n  nt rm NT20260528T143012\n"),
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
        "help" => Ok("nt help [command...]\n\nShow command help.\n\nExamples:\n  nt help find\n"),
        _ => Err(NtError::Message(format!(
            "unknown help topic `{key}`; run `nt help`"
        ))),
    }
}

const ROOT: &str = r#"nt

Markdown-first CLI note organizer.

Usage:
  nt <command> [positional...]

Commands:
  init        create a vault
  add         add a Markdown note
  rebuild     rebuild active vault index
  list        list notes and metadata projections
  find        find notes by query expressions
  show        show one exact note
  open        edit one note with $EDITOR
  rm          remove one note
  update      update one metadata field
  agenda      show actionable todos
  export      export Markdown with front matter
  config      inspect or select vaults
  completion  generate shell completion
  help        show help

Examples:
  nt init notes
  nt list
  nt find tag:decision qemu
  nt agenda today
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

const LIST: &str = r#"nt list
nt list <field>[,<field>...] [filter...]
nt list ids
nt list titles
nt list tags [tag]
nt list collections [collection]
nt list links <id> [from|to]

Print active-vault metadata rows with optional structured filters. Fields include
id, path, created, updated, title, kind, status, priority, scheduled, due,
closed, tag, collection, link, and source. Bare list prints every field.

Examples:
  nt list id
  nt list id,title,status status:open
  nt list title,tag collection:projects/nt
  nt list tags storage
  nt list collections projects/nt
  nt list links NT20260528T143012 from
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

#[cfg(test)]
mod tests {
    use super::topic_text;

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
    fn legacy_topics_are_unknown() {
        for topic in ["ids", "tags", "collection", "status", "link"] {
            assert!(topic_text(topic).is_err());
        }
    }
}

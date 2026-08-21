use std::io::Write;

use crate::error::{NtError, Result};

pub fn print(topic: &[String], output: &mut dyn Write) -> Result<()> {
    output.write_all(topic_text(&topic.join(" "))?.as_bytes())?;
    Ok(())
}

fn topic_text(key: &str) -> Result<&'static str> {
    match key {
        "" => Ok(ROOT),
        "init" => Ok("nt init\n\nInitialize $HOME/.nt/nt.sqlite3.\n"),
        "add" => Ok(
            "nt add [metadata...] [-- body...]\n\nAdd a CommonMark note from trailing text, stdin, or $VISUAL/$EDITOR.\n",
        ),
        "show" => Ok("nt show <id>\n\nPrint the exact canonical note body.\n"),
        "list" => Ok(
            "nt list [filter...]\nnt list tags\nnt list collections\n\nList complete fixed note summaries or current metadata values. links-to:<target> selects notes pointing to a target; linked-from:<source> selects notes pointed to by a source. Use limit:<n> for an explicit result bound.\n",
        ),
        "find" => Ok(
            "nt find <term-or-filter...>\n\nFind complete note summaries using literal lexical terms and structured filters. links-to:<target> selects notes pointing to a target; linked-from:<source> selects notes pointed to by a source. Use limit:<n> for an explicit result bound.\n",
        ),
        "rm" => Ok("nt rm <id...>\n\nRemove notes atomically.\n"),
        "edit" => Ok("nt edit <id> [-- body...]\n\nReplace a complete note body.\n"),
        "move" => Ok("nt move <id> <collection>\n\nMove a note to one collection.\n"),
        "tag" => Ok("nt tag <id> <+tag|-tag>\n\nAdd or remove one tag.\n"),
        "link" => Ok("nt link <id> <+id|-id>\n\nAdd or remove one directional note link.\n"),
        "library" => Ok(
            "nt library <command> [args...]\n\nStore external resources, immutable captures, and capture summaries.\n",
        ),
        "library add" => Ok(
            "nt library add <source> <title...>\n\nCreate or resolve a Library item and capture content from stdin or $VISUAL/$EDITOR.\n",
        ),
        "library capture" => Ok(
            "nt library capture <library-id>\n\nAppend changed content from stdin or $VISUAL/$EDITOR. Identical content is idempotent.\n",
        ),
        "library show" => {
            Ok("nt library show <library-id>\n\nPrint the exact latest captured content.\n")
        }
        "library find" => Ok(
            "nt library find <term-or-filter...>\n\nSearch only the latest capture of each Library item. Filters: id:, source:, title:, text:, since:, before:, limit:.\n",
        ),
        "library summary" => Ok(
            "nt library summary <library-id>\n\nReplace the manual summary for the latest capture from stdin or $VISUAL/$EDITOR.\n",
        ),
        "library history" => Ok(
            "nt library history <library-id>\n\nList immutable capture metadata and capture-specific summaries.\n",
        ),
        "ref" => Ok(
            "nt ref <note-id> <library-id>\n\nReference a Library item as evidence for a note.\n",
        ),
        "unref" => Ok("nt unref <note-id> <library-id>\n\nRemove a note's evidence reference.\n"),
        "help" => Ok("nt help [command...]\n\nShow command help.\n"),
        _ => Err(NtError::UnknownHelpTopic(key.to_string())),
    }
}

const ROOT: &str = r#"nt

Local, agent-first knowledge layer.

Usage:
  nt <command> [args...]

Commands:
  init                                 initialize canonical storage
  add [metadata...] [-- body...]       add a CommonMark note
  show <id>                            print one exact body
  list [filter...]|tags|collections    list note summaries or metadata values
  find <term-or-filter...>             search note summaries
  rm <id...>                           remove notes atomically
  edit <id> [-- body...]               replace one body
  move <id> <collection>               move one note
  tag <id> <+tag|-tag>                 change one tag
  link <id> <+id|-id>                  change one directional link
  library <command> [args...]           manage external evidence and captures
  ref <note-id> <library-id>            reference Library evidence from a note
  unref <note-id> <library-id>          remove a Library evidence reference
  help [command...]                    show command help
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};
    use crate::error::NtError;

    #[test]
    fn root_lists_only_clean_sheet_commands() {
        for command in [
            "init", "add", "show", "list", "find", "rm", "edit", "move", "tag", "link", "library",
            "ref", "unref",
        ] {
            assert!(ROOT.contains(command));
            assert!(topic_text(command).is_ok());
        }
        for removed in ["todo", "agenda", "vault", "export", "update"] {
            assert!(!ROOT.contains(removed));
        }
    }

    #[test]
    fn unknown_topics_are_errors() {
        assert!(matches!(
            topic_text("search"),
            Err(NtError::UnknownHelpTopic(topic)) if topic == "search"
        ));
    }
}

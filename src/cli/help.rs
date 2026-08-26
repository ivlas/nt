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
        "read" => Ok(
            "nt read [filter...]\n\nStream complete notes selected by the same structured filters as nt list. Repeat full id:<id> expressions to batch arbitrary notes, or use id:- with one full ID per stdin line for batches too large for command arguments. Duplicates and missing IDs are omitted, and canonical result ordering is preserved. Redirected output is JSONL. Use limit:<n> for an explicit result bound.\n",
        ),
        "changes" => Ok(
            "nt changes since:<revision>\n\nStream canonical note changes committed strictly after a global revision. Redirected output is JSONL.\n",
        ),
        "find" => Ok(
            "nt find <term-or-filter...>\n\nFind complete note summaries using literal lexical terms and structured filters. links-to:<target> selects notes pointing to a target; linked-from:<source> selects notes pointed to by a source. Use limit:<n> for an explicit result bound.\n",
        ),
        "rm" => Ok("nt rm <id...>\n\nRemove notes atomically.\n"),
        "edit" => Ok(
            "nt edit <id> [if-rev:<revision>] [-- body...]\n\nReplace a complete note body. if-rev applies the edit only while the note remains at the observed revision.\n",
        ),
        "move" => Ok(
            "nt move <id> <collection> [if-rev:<revision>]\n\nMove a note to one collection. if-rev applies the move only while the note remains at the observed revision.\n",
        ),
        "tag" => Ok(
            "nt tag <id> <+tag|-tag> [if-rev:<revision>]\n\nAdd or remove one tag. if-rev applies the change only while the note remains at the observed revision.\n",
        ),
        "link" => Ok(
            "nt link <id> <+id|-id> [if-rev:<revision>]\n\nAdd or remove one directional note link. if-rev applies the change only while the source note remains at the observed revision.\n",
        ),
        "help" => Ok("nt help [command...]\n\nShow command help.\n"),
        _ => Err(NtError::UnknownHelpTopic(key.to_string())),
    }
}

const ROOT: &str = r#"nt

Local, agent-first CommonMark notes.

Usage:
  nt <command> [args...]

Commands:
  init                                 initialize canonical storage
  add [metadata...] [-- body...]       add a CommonMark note
  show <id>                            print one exact body
  list [filter...]|tags|collections    list note summaries or metadata values
  read [filter...]                     stream complete notes
  changes since:<revision>             stream canonical changes
  find <term-or-filter...>             search note summaries
  rm <id...>                           remove notes atomically
  edit <id> [if-rev:<revision>] [-- body...]
                                       replace one body
  move <id> <collection> [if-rev:<revision>]
                                       move one note
  tag <id> <+tag|-tag> [if-rev:<revision>]
                                       change one tag
  link <id> <+id|-id> [if-rev:<revision>]
                                       change one directional link
  help [command...]                    show command help
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};
    use crate::error::NtError;

    #[test]
    fn root_lists_only_clean_sheet_commands() {
        for command in [
            "init", "add", "show", "list", "read", "changes", "find", "rm", "edit", "move", "tag",
            "link",
        ] {
            assert!(ROOT.contains(command));
            assert!(topic_text(command).is_ok());
        }
        for removed in ["memory", "todo", "agenda", "vault", "export", "update"] {
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

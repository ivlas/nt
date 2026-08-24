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
        "memory" => Ok(
            "nt memory <command> [args...]\n\nAppend immutable experience, compress it into a binary tree, and read it by age or exact pattern. Run nt help memory <command> for details.\n",
        ),
        "memory add" => Ok(
            "nt memory add [memory...]\n\nAppend one immutable single-line memory from arguments or stdin.\n",
        ),
        "memory wake" => Ok(
            "nt memory wake\n\nPrint a bounded chronological view: old history is summarized and recent history is precise.\n",
        ),
        "memory recall" => Ok(
            "nt memory recall <pattern...>\n\nScan immutable raw history for a case-sensitive literal substring.\n",
        ),
        "memory nap" => Ok(
            "nt memory nap\nnt memory nap <range> [summary...]\n\nPrint the next derived compression task, or store a caller-produced summary.\n",
        ),
        "memory zoom" => {
            Ok("nt memory zoom <range>\n\nReveal the two direct children of a summary range.\n")
        }
        "memory forget" => Ok(
            "nt memory forget <range>\n\nDelete one derived summary and its ancestors without changing raw history.\n",
        ),
        "help" => Ok("nt help [command...]\n\nShow command help.\n"),
        _ => Err(NtError::UnknownHelpTopic(key.to_string())),
    }
}

const ROOT: &str = r#"nt

Local, agent-first notes and persistent memory.

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
  memory <command> [args...]           use immutable persistent memory
  help [command...]                    show command help
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};
    use crate::error::NtError;

    #[test]
    fn root_lists_only_clean_sheet_commands() {
        for command in [
            "init", "add", "show", "list", "find", "rm", "edit", "move", "tag", "link", "memory",
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

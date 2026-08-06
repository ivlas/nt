use crate::error::{NtError, Result};

pub fn print(topic: &[String]) -> Result<()> {
    print!("{}", topic_text(&topic.join(" "))?);
    Ok(())
}

fn topic_text(key: &str) -> Result<&'static str> {
    match key {
        "" => Ok(ROOT),
        "init" => Ok("nt init\n\nInitialize $HOME/.nt/nt.sqlite3.\n"),
        "add" => Ok(
            "nt add [metadata...] [-- body...]\n\nAdd a CommonMark note from trailing text, stdin, or $EDITOR.\n",
        ),
        "show" => Ok("nt show <id>\n\nPrint the exact canonical note body.\n"),
        "list" => {
            Ok("nt list [filter...]\n\nList fixed note summaries using structured filters.\n")
        }
        "find" => Ok(
            "nt find <term-or-filter...>\n\nFind note summaries using literal lexical terms and structured filters.\n",
        ),
        "rm" => Ok("nt rm <id...>\n\nRemove notes atomically.\n"),
        "edit" => Ok("nt edit <id> [-- body...]\n\nReplace a complete note body.\n"),
        "move" => Ok("nt move <id> <collection>\n\nMove a note to one collection.\n"),
        "tag" => Ok("nt tag <id> <+tag|-tag>\n\nAdd or remove one tag.\n"),
        "link" => Ok("nt link <id> <+id|-id>\n\nAdd or remove one directional note link.\n"),
        "help" => Ok("nt help [command...]\n\nShow command help.\n"),
        _ => Err(NtError::Message(format!(
            "unknown help topic `{key}`; run nt help"
        ))),
    }
}

const ROOT: &str = r#"nt

Local, agent-first note layer.

Usage:
  nt <command> [args...]

Commands:
  init                                 initialize canonical storage
  add [metadata...] [-- body...]       add a CommonMark note
  show <id>                            print one exact body
  list [filter...]                     list fixed note summaries
  find <term-or-filter...>             search note summaries
  rm <id...>                           remove notes atomically
  edit <id> [-- body...]               replace one body
  move <id> <collection>               move one note
  tag <id> <+tag|-tag>                 change one tag
  link <id> <+id|-id>                  change one directional link
  help [command...]                    show command help
"#;

#[cfg(test)]
mod tests {
    use super::{ROOT, topic_text};

    #[test]
    fn root_lists_only_clean_sheet_commands() {
        for command in [
            "init", "add", "show", "list", "find", "rm", "edit", "move", "tag", "link",
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
        assert!(topic_text("search").is_err());
    }
}
